use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_client::ipc::send_raw;
use orbit_core::{
    audit::events_for_plan,
    eval::EvalConstraint,
    ipc::{PlanStreamEvent, ProjectRole, Request, Response},
    memory::find_run,
    plan::{CrossRepoSpec, Plan, PlanNodeType},
    template,
};
use serde::Serialize;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

#[derive(Debug, Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub command: Option<PlanCommand>,

    /// Intent for plan creation (used when no subcommand given)
    pub intent: Option<String>,

    /// Preview the plan without executing it
    #[arg(long)]
    pub dry_run: bool,

    /// Print system prompt, user prompt, and raw LLM response
    #[arg(long)]
    pub verbose: bool,

    /// Block and stream live plan events until the plan completes
    #[arg(long)]
    pub foreground: bool,

    /// Workspace scope override
    #[arg(long)]
    pub workspace: Option<String>,

    /// Tenant scope override
    #[arg(long)]
    pub tenant: Option<String>,

    /// Project scope override
    #[arg(long)]
    pub project: Option<String>,

    /// Repository scope override
    #[arg(long)]
    pub repository: Option<String>,

    /// Additional repos available for cross-repo node targeting (path to local repo dir)
    #[arg(long = "repo", value_name = "PATH")]
    pub extra_repos: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// Show plan details
    Get { id: String },
    /// List all plans
    List,
    /// Cancel a running plan
    Cancel { id: String },
    /// Show recent plan history
    History {
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Approve an AwaitingApproval node
    Approve {
        /// Plan ID
        plan_id: String,
        /// Node ID to approve
        node_id: String,
    },
    /// Show aggregate audit statistics
    Stats,
    /// Export a plan bundle (plan JSON + audit trail + memory record)
    Export {
        /// Plan ID to export
        id: String,
        /// Write to stdout instead of a file
        #[arg(long)]
        stdout: bool,
        /// Export as readable markdown report instead of JSON
        #[arg(long)]
        markdown: bool,
    },
    /// Re-execute a plan from its failed nodes without re-planning
    Retry {
        /// Plan ID to retry
        id: String,
    },
    /// Poll a plan's status until it reaches a terminal state
    Watch {
        /// Plan ID to watch
        id: String,
        /// Poll interval in seconds (default: 3)
        #[arg(long, default_value = "3")]
        interval: u64,
    },
    /// Compare two plans node-by-node (useful for inspecting replans)
    Diff {
        /// First plan ID
        id_a: String,
        /// Second plan ID
        id_b: String,
    },
    /// Delete terminal plans older than N days
    Prune {
        /// Delete plans older than this many days (default: 7)
        #[arg(long, default_value = "7")]
        days: u64,
        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,
    },
    /// Freeze dispatch — Running nodes finish but no new nodes are started
    Pause {
        /// Plan ID to pause
        id: String,
    },
    /// Resume a paused plan
    Resume {
        /// Plan ID to resume
        id: String,
    },
    /// Create a restricted project socket at the given path
    Socket {
        /// Path for the new socket file (e.g. .orbit/orbit.sock)
        path: String,
        /// Grant observer (read-only) access instead of contributor (read + approve)
        #[arg(long)]
        observer: bool,
    },
    /// View captured output logs for a plan node
    Logs {
        /// Plan ID
        id: String,
        /// Node ID (e.g. node_1)
        node_id: String,
        /// Print only the last N lines
        #[arg(long)]
        tail: Option<usize>,
        /// Follow new output as it is written (like tail -f)
        #[arg(long)]
        follow: bool,
    },
    /// List all distinct repos targeted by a plan's nodes
    Repos {
        /// Plan ID
        id: String,
    },
    /// Dry-run planner and evaluate the plan structure (no engine executed)
    Eval {
        /// Intent to plan
        intent: String,
        /// Require specific node types (comma-separated: code,test,review,verify,pr)
        #[arg(long, value_delimiter = ',')]
        expect: Vec<String>,
        /// Minimum number of nodes
        #[arg(long)]
        min_nodes: Option<usize>,
        /// Maximum number of nodes
        #[arg(long)]
        max_nodes: Option<usize>,
        /// Fail if any node has no verify strategy
        #[arg(long)]
        require_verify: bool,
        /// Workspace scope override
        #[arg(long)]
        workspace: Option<String>,
        /// Tenant scope override
        #[arg(long)]
        tenant: Option<String>,
        /// Project scope override
        #[arg(long)]
        project: Option<String>,
        /// Repository scope override
        #[arg(long)]
        repository: Option<String>,
    },
    /// Manage and run plan templates from ~/.config/orbit/plans/
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// List all available templates
    List,
    /// Show a template's content and variables
    Show {
        /// Template name
        name: String,
    },
    /// Run a template, substituting any {{variable}} placeholders
    Run {
        /// Template name
        name: String,
        /// Variable substitutions in key=value format
        vars: Vec<String>,
        /// Preview the plan without executing
        #[arg(long)]
        dry_run: bool,
        /// Block and stream live output until the plan completes
        #[arg(long)]
        foreground: bool,
        /// Workspace scope override
        #[arg(long)]
        workspace: Option<String>,
        /// Tenant scope override
        #[arg(long)]
        tenant: Option<String>,
        /// Project scope override
        #[arg(long)]
        project: Option<String>,
        /// Repository scope override
        #[arg(long)]
        repository: Option<String>,
    },
    /// Create a new template (opens $EDITOR)
    Create {
        /// Template name (kebab-case recommended)
        name: String,
    },
    /// Save a plan's intent as a new template
    FromPlan {
        /// Plan ID to capture the intent from
        plan_id: String,
        /// Template name to save as
        name: String,
        /// One-line description for the template
        #[arg(long, default_value = "")]
        description: String,
    },
}

pub async fn run(args: PlanArgs) -> Result<()> {
    match args.command {
        Some(PlanCommand::Get { id }) => {
            match send_raw(&Request::GetPlan { id }).await? {
                Response::PlanInfo { plan } => {
                    println!("Plan:   {} [{:?}]", plan.id, plan.status);
                    println!("Intent: {}", plan.intent);
                    println!("Nodes ({}):", plan.nodes.len());
                    let plan_suffix = plan.id.trim_start_matches("plan_");
                    let mut total_cost = 0.0f64;
                    for node in &plan.nodes {
                        let cost_str = if let Some(u) = &node.token_usage {
                            total_cost += u.estimated_cost_usd;
                            format!("  ~${:.4}", u.estimated_cost_usd)
                        } else {
                            String::new()
                        };
                        let repo_tag = if let Some(ref s) = node.scope_override {
                            s.repository.as_deref().map(|r| format!(" [repo→{r}]")).unwrap_or_default()
                        } else {
                            String::new()
                        };
                        println!(
                            "  [{:?}] {} — {:?}{}{}",
                            node.status, node.label, node.task_type, cost_str, repo_tag
                        );
                        let session_key = format!("orbit-plan-{plan_suffix}-{}", node.id);
                        if node.status == orbit_core::plan::NodeStatus::Running {
                            println!("         tmux attach -t {session_key}");
                        }
                        // Log preview: last 5 lines for Running/Completed/Failed nodes
                        if matches!(
                            node.status,
                            orbit_core::plan::NodeStatus::Running
                                | orbit_core::plan::NodeStatus::Completed
                                | orbit_core::plan::NodeStatus::Failed
                        ) && let Some(preview) = node_log_preview(&session_key, 5)
                        {
                            for line in preview.lines() {
                                println!("         │ {line}");
                            }
                        }
                    }
                    if total_cost > 0.0 {
                        println!("Cost:   ~${total_cost:.4} estimated (Claude Sonnet pricing)");
                    }
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::List) => {
            match send_raw(&Request::ListPlans).await? {
                Response::Plans { plans } => {
                    if plans.is_empty() {
                        println!("No plans found.");
                    } else {
                        print_plan_tree(&plans);
                    }
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Cancel { id }) => {
            match send_raw(&Request::CancelPlan { id }).await? {
                Response::PlanCancelled { id } => println!("Plan {id} cancelled."),
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::History { limit }) => {
            let runs = orbit_core::memory::load_recent_runs(limit);
            if runs.is_empty() {
                println!("No plan history found.");
                return Ok(());
            }
            for run in runs.iter().rev() {
                println!(
                    "{} [{}] {} node(s), {} replan(s) — {}",
                    run.plan_id, run.outcome, run.node_count, run.replan_count, run.intent
                );
            }
        }

        Some(PlanCommand::Approve { plan_id, node_id }) => {
            match send_raw(&Request::ApprovePlanNode { plan_id, node_id }).await? {
                Response::PlanApproved { plan_id, node_id } => {
                    println!("Approved: node {node_id} in plan {plan_id}");
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Retry { id }) => {
            match send_raw(&Request::RetryPlan { id }).await? {
                Response::PlanRetried { id, reset_count } => {
                    println!("Plan {id} retried: {reset_count} failed node(s) reset to Pending.");
                    println!("Running. Check status with: orbit plan get {id}");
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Watch { id, interval: _ }) => {
            println!("Streaming plan {id}… (Ctrl+C to detach)");
            let mut rx = orbit_client::ipc::stream_plan(&id).await?;
            while let Some(event) = rx.recv().await {
                match &event {
                    PlanStreamEvent::NodeStarted { node_id, label, .. } => {
                        println!("[start]  {node_id}: {label}");
                    }
                    PlanStreamEvent::NodeCompleted { node_id, .. } => {
                        println!("[done]   {node_id}");
                    }
                    PlanStreamEvent::NodeFailed { node_id, error, .. } => {
                        println!("[fail]   {node_id}: {error}");
                    }
                    PlanStreamEvent::PlanCompleted { .. } => {
                        println!("Plan {id} completed.");
                    }
                    PlanStreamEvent::PlanFailed { .. } => {
                        println!("Plan {id} failed.");
                        std::process::exit(1);
                    }
                    PlanStreamEvent::PlanReplanning { child_plan_id, .. } => {
                        println!("[replan] → {child_plan_id}");
                    }
                }
            }
        }

        Some(PlanCommand::Stats) => {
            match send_raw(&Request::GetPlanStats).await? {
                Response::PlanStats { stats } => {
                    println!("Plans:  {} total, {} completed, {} failed",
                        stats.total_plans, stats.completed_plans, stats.failed_plans);
                    println!("Nodes:  {} dispatched, {} completed, {} failed",
                        stats.total_nodes_dispatched, stats.total_nodes_completed, stats.total_nodes_failed);
                    println!("Avg duration: {}s", stats.avg_duration_secs);
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Export { id, stdout, markdown }) => {
            let plan = Plan::load(&id).map_err(|e| anyhow::anyhow!("plan not found: {e}"))?;
            let audit_trail = events_for_plan(&id);
            let memory_run = find_run(&id);

            if markdown {
                let md = render_plan_markdown(&plan, &audit_trail, memory_run.as_ref());
                if stdout {
                    println!("{md}");
                } else {
                    let filename = format!("orbit-export-{id}.md");
                    std::fs::write(&filename, &md)?;
                    println!("Exported to {filename}");
                }
            } else {
                #[derive(Serialize)]
                struct PlanExportBundle<'a> {
                    plan: &'a Plan,
                    audit_trail: &'a Vec<orbit_core::audit::AuditEvent>,
                    memory_run: Option<&'a orbit_core::memory::PlanRunRecord>,
                }

                let bundle = PlanExportBundle { plan: &plan, audit_trail: &audit_trail, memory_run: memory_run.as_ref() };
                let json = serde_json::to_string_pretty(&bundle)?;

                if stdout {
                    println!("{json}");
                } else {
                    let filename = format!("orbit-export-{id}.json");
                    std::fs::write(&filename, &json)?;
                    println!("Exported to {filename}");
                    println!(
                        "  {} node(s), {} audit event(s){}",
                        plan.nodes.len(),
                        audit_trail.len(),
                        if memory_run.is_some() { ", memory record included" } else { "" }
                    );
                }
            }
        }

        Some(PlanCommand::Diff { id_a, id_b }) => {
            let a = Plan::load(&id_a).map_err(|e| anyhow::anyhow!("{id_a}: {e}"))?;
            let b = Plan::load(&id_b).map_err(|e| anyhow::anyhow!("{id_b}: {e}"))?;

            println!("Diff: {} → {}", a.id, b.id);
            println!("  A: [{:?}]  {}", a.status, a.intent);
            println!("  B: [{:?}]  {}", b.status, b.intent);
            println!();

            // Index nodes by label for matching across replans
            let a_nodes: std::collections::HashMap<&str, _> =
                a.nodes.iter().map(|n| (n.label.as_str(), n)).collect();
            let b_nodes: std::collections::HashMap<&str, _> =
                b.nodes.iter().map(|n| (n.label.as_str(), n)).collect();

            // Added in B
            for (label, nb) in &b_nodes {
                if !a_nodes.contains_key(label) {
                    println!("  + [{:?}] {label}  ({:?})", nb.status, nb.task_type);
                }
            }
            // Removed in B
            for (label, na) in &a_nodes {
                if !b_nodes.contains_key(label) {
                    println!("  - [{:?}] {label}  ({:?})", na.status, na.task_type);
                }
            }
            // Changed status
            for (label, na) in &a_nodes {
                if let Some(nb) = b_nodes.get(label) {
                    if na.status != nb.status {
                        println!("  ~ {label}  [{:?}] → [{:?}]", na.status, nb.status);
                    } else {
                        println!("    {label}  [{:?}]", na.status);
                    }
                }
            }

            let a_cost: f64 = a.nodes.iter().filter_map(|n| n.token_usage.as_ref()).map(|u| u.estimated_cost_usd).sum();
            let b_cost: f64 = b.nodes.iter().filter_map(|n| n.token_usage.as_ref()).map(|u| u.estimated_cost_usd).sum();
            if a_cost > 0.0 || b_cost > 0.0 {
                println!();
                println!("  Cost: A ~${a_cost:.4}  →  B ~${b_cost:.4}");
            }
        }

        Some(PlanCommand::Prune { days, dry_run }) => {
            let cutoff_secs = days * 24 * 3600;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let plans = Plan::load_all();
            let prunable: Vec<&Plan> = plans
                .iter()
                .filter(|p| {
                    matches!(
                        p.status,
                        orbit_core::plan::PlanStatus::Completed
                            | orbit_core::plan::PlanStatus::Failed
                            | orbit_core::plan::PlanStatus::Cancelled
                    ) && now.saturating_sub(p.created_at) >= cutoff_secs
                })
                .collect();

            if prunable.is_empty() {
                println!("No plans to prune (terminal, older than {days} days).");
                return Ok(());
            }

            for plan in &prunable {
                let age_days = now.saturating_sub(plan.created_at) / 86400;
                println!(
                    "{} [{:?}] {}d old — {}",
                    plan.id, plan.status, age_days, plan.intent
                );
                if !dry_run {
                    let _ = plan.delete();
                }
            }

            if dry_run {
                println!("\n(dry-run) {} plan(s) would be deleted.", prunable.len());
            } else {
                println!("\nDeleted {} plan(s).", prunable.len());
            }
        }

        Some(PlanCommand::Pause { id }) => {
            match send_raw(&Request::PausePlan { id }).await? {
                Response::PlanPaused { id } => {
                    println!("Plan {id} paused. Running nodes will finish; no new nodes will start.");
                    println!("Resume with: orbit plan resume {id}");
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Resume { id }) => {
            match send_raw(&Request::ResumePlan { id }).await? {
                Response::PlanResumed { id } => {
                    println!("Plan {id} resumed.");
                    println!("Stream live output with: orbit plan watch {id}");
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Socket { path, observer }) => {
            let role = if observer { ProjectRole::Observer } else { ProjectRole::Contributor };
            match send_raw(&Request::AddProjectSocket { path: path.clone(), role }).await? {
                Response::ProjectSocketAdded { path } => {
                    let role_name = if observer { "observer" } else { "contributor" };
                    println!("Socket created ({role_name}): {path}");
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Logs { id, node_id, tail, follow }) => {
            let plan_suffix = id.trim_start_matches("plan_");
            let session_key = format!("orbit-plan-{plan_suffix}-{node_id}");
            let log_path = std::env::temp_dir()
                .join("orbit-plan-nodes")
                .join(format!("{session_key}.log"));

            if !log_path.exists() {
                eprintln!("No log found for node {node_id} in plan {id}");
                eprintln!("Expected: {}", log_path.display());
                std::process::exit(1);
            }

            if follow {
                let mut file = std::fs::File::open(&log_path)?;
                // Print existing content first
                let mut reader = BufReader::new(&file);
                let mut buf = String::new();
                while reader.read_line(&mut buf)? > 0 {
                    print!("{buf}");
                    buf.clear();
                }
                // Then follow for new lines
                let mut pos = file.stream_position()?;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let new_len = file.metadata()?.len();
                    if new_len > pos {
                        file.seek(SeekFrom::Start(pos))?;
                        let mut reader = BufReader::new(&file);
                        let mut line = String::new();
                        while reader.read_line(&mut line)? > 0 {
                            print!("{line}");
                            line.clear();
                        }
                        pos = file.stream_position()?;
                    }
                }
            } else {
                let content = std::fs::read_to_string(&log_path)?;
                if let Some(n) = tail {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = lines.len().saturating_sub(n);
                    for line in &lines[start..] {
                        println!("{line}");
                    }
                } else {
                    print!("{content}");
                }
            }
        }

        Some(PlanCommand::Eval {
            intent,
            expect,
            min_nodes,
            max_nodes,
            require_verify,
            workspace,
            tenant,
            project,
            repository,
        }) => {
            let (workspace, tenant, project, repository) = if workspace.is_none()
                && tenant.is_none()
                && project.is_none()
                && repository.is_none()
            {
                resolve_scope_from_cwd()
            } else {
                (workspace, tenant, project, repository)
            };

            let mut constraints: Vec<EvalConstraint> = vec![];

            for ty_str in &expect {
                let node_type = match ty_str.to_lowercase().as_str() {
                    "code" => PlanNodeType::Code,
                    "test" => PlanNodeType::Test,
                    "review" => PlanNodeType::Review,
                    "verify" => PlanNodeType::Verify,
                    "pr" => PlanNodeType::Pr,
                    other => PlanNodeType::Custom(other.to_string()),
                };
                constraints.push(EvalConstraint::HasNodeType { node_type });
            }
            if let Some(n) = min_nodes {
                constraints.push(EvalConstraint::MinNodes { count: n });
            }
            if let Some(n) = max_nodes {
                constraints.push(EvalConstraint::MaxNodes { count: n });
            }
            if require_verify {
                constraints.push(EvalConstraint::NodesHaveVerify);
            }

            match send_raw(&Request::EvalPlan {
                intent: intent.clone(),
                workspace,
                tenant,
                project,
                repository,
                constraints,
            })
            .await?
            {
                Response::PlanEvalResult { plan, result } => {
                    println!("Plan: {} ({} node(s))", plan.id, plan.nodes.len());
                    for node in &plan.nodes {
                        println!("  {:?} — {}", node.task_type, node.label);
                    }
                    println!();
                    let status = if result.passed { "PASS" } else { "FAIL" };
                    println!("Eval: {status}");
                    for check in &result.checks {
                        let mark = if check.passed { "✓" } else { "✗" };
                        println!("  {mark} {} — {}", check.name, check.detail);
                    }
                    if !result.passed {
                        std::process::exit(1);
                    }
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Repos { id }) => {
            match send_raw(&Request::GetPlan { id }).await? {
                Response::PlanInfo { plan } => {
                    let mut repos: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                    // Include the plan's own scope if it has a repository
                    if let Some(ref r) = plan.scope.repository {
                        repos.insert(r.clone());
                    }
                    for node in &plan.nodes {
                        if let Some(ref s) = node.scope_override {
                            if let Some(ref r) = s.repository {
                                repos.insert(r.clone());
                            }
                        }
                    }
                    if repos.is_empty() {
                        println!("Plan {} has no explicit repo scopes.", plan.id);
                    } else {
                        println!("Repos touched by plan {}:", plan.id);
                        for r in &repos {
                            println!("  {r}");
                        }
                    }
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        Some(PlanCommand::Template { command }) => {
            run_template(command).await?;
        }

        None => {
            let intent = match args.intent {
                Some(i) => i,
                None => {
                    eprintln!("Usage: orbit plan \"<intent>\" [--dry-run]");
                    std::process::exit(1);
                }
            };

            let (workspace, tenant, project, repository) = if args.workspace.is_none()
                && args.tenant.is_none()
                && args.project.is_none()
                && args.repository.is_none()
            {
                resolve_scope_from_cwd()
            } else {
                (args.workspace, args.tenant, args.project, args.repository)
            };

            let extra_repos: Vec<CrossRepoSpec> = args.extra_repos
                .iter()
                .map(|p| {
                    let path = std::path::Path::new(p);
                    let alias = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(p)
                        .to_string();
                    CrossRepoSpec {
                        alias: alias.clone(),
                        workspace: None,
                        tenant: None,
                        project: None,
                        repository: Some(alias),
                    }
                })
                .collect();

            println!("Planning: {intent}");
            if args.dry_run {
                println!("(dry-run — plan will not execute)");
            }

            match send_raw(&Request::CreatePlan {
                intent: intent.clone(),
                workspace,
                tenant,
                project,
                repository,
                dry_run: args.dry_run,
                verbose: args.verbose,
                extra_repos,
            })
            .await?
            {
                Response::PlanCreated { id, node_count, trace } => {
                    if let Some(t) = trace {
                        println!("── system prompt ────────────────────────────────────");
                        println!("{}", t.system_prompt);
                        println!("── user prompt ──────────────────────────────────────");
                        println!("{}", t.user_prompt);
                        println!("── raw LLM response ─────────────────────────────────");
                        println!("{}", t.raw_response);
                        println!("─────────────────────────────────────────────────────");
                    }
                    println!("Plan created: {id} ({node_count} node(s))");
                    if !args.dry_run {
                        // Auto-create a project socket in cwd/.orbit/ so contributors
                        // can approve nodes or read plan status without owner access.
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let orbit_dir = cwd.join(".orbit");
                        if orbit_dir.exists() || std::fs::create_dir_all(&orbit_dir).is_ok() {
                            let sock_path = orbit_dir.join("orbit.sock");
                            let _ = send_raw(&Request::AddProjectSocket {
                                path: sock_path.to_string_lossy().into_owned(),
                                role: ProjectRole::Contributor,
                            }).await;
                        }

                        if args.foreground {
                            stream_until_done(&id).await;
                        } else {
                            println!("Running. Check status with: orbit plan get {id}");
                            println!("Stream live output with:    orbit plan watch {id}");
                        }
                    }
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }
    }

    Ok(())
}

async fn stream_until_done(id: &str) {
    match orbit_client::ipc::stream_plan(id).await {
        Err(e) => {
            eprintln!("stream error: {e}");
            std::process::exit(1);
        }
        Ok(mut rx) => {
            while let Some(event) = rx.recv().await {
                match &event {
                    PlanStreamEvent::NodeStarted { node_id, label, .. } => {
                        println!("[start]  {node_id}: {label}");
                    }
                    PlanStreamEvent::NodeCompleted { node_id, .. } => {
                        println!("[done]   {node_id}");
                    }
                    PlanStreamEvent::NodeFailed { node_id, error, .. } => {
                        println!("[fail]   {node_id}: {error}");
                    }
                    PlanStreamEvent::PlanCompleted { plan_id } => {
                        println!("Plan {plan_id} completed.");
                    }
                    PlanStreamEvent::PlanFailed { plan_id } => {
                        println!("Plan {plan_id} failed.");
                        std::process::exit(1);
                    }
                    PlanStreamEvent::PlanReplanning { child_plan_id, .. } => {
                        println!("[replan] → {child_plan_id}");
                    }
                }
            }
        }
    }
}

fn print_plan_tree(plans: &[Plan]) {
    fn print_node(plan: &Plan, all: &[Plan], prefix: &str, is_last: bool) {
        let connector = if is_last { "└── " } else { "├── " };
        let status_icon = match plan.status {
            orbit_core::plan::PlanStatus::Completed => "✓",
            orbit_core::plan::PlanStatus::Failed => "✗",
            orbit_core::plan::PlanStatus::Cancelled => "⊘",
            orbit_core::plan::PlanStatus::Running => "▶",
            _ => "·",
        };
        println!(
            "{}{}{} {} [{:?}] {} node(s) — {}",
            prefix,
            connector,
            status_icon,
            plan.id,
            plan.status,
            plan.nodes.len(),
            plan.intent
        );
        let child_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });
        let children: Vec<&Plan> = all
            .iter()
            .filter(|p| p.parent_plan_id.as_deref() == Some(&plan.id))
            .collect();
        for (i, child) in children.iter().enumerate() {
            print_node(child, all, &child_prefix, i == children.len() - 1);
        }
    }

    let roots: Vec<&Plan> = plans
        .iter()
        .filter(|p| p.parent_plan_id.is_none())
        .collect();

    for (i, root) in roots.iter().enumerate() {
        let is_last = i == roots.len() - 1;
        let status_icon = match root.status {
            orbit_core::plan::PlanStatus::Completed => "✓",
            orbit_core::plan::PlanStatus::Failed => "✗",
            orbit_core::plan::PlanStatus::Cancelled => "⊘",
            orbit_core::plan::PlanStatus::Running => "▶",
            _ => "·",
        };
        println!(
            "{} {} [{:?}] {} node(s) — {}",
            status_icon,
            root.id,
            root.status,
            root.nodes.len(),
            root.intent
        );
        let children: Vec<&Plan> = plans
            .iter()
            .filter(|p| p.parent_plan_id.as_deref() == Some(&root.id))
            .collect();
        for (j, child) in children.iter().enumerate() {
            print_node(child, plans, "", j == children.len() - 1);
        }
        if !is_last {
            println!();
        }
    }
}

fn render_plan_markdown(
    plan: &Plan,
    audit_trail: &[orbit_core::audit::AuditEvent],
    memory_run: Option<&orbit_core::memory::PlanRunRecord>,
) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Plan: {}\n\n", plan.id));
    md.push_str(&format!("**Intent:** {}\n\n", plan.intent));
    md.push_str(&format!("**Status:** {:?}\n\n", plan.status));

    let scope = &plan.scope;
    if scope.workspace.is_some() || scope.tenant.is_some() {
        let parts: Vec<String> = [
            scope.workspace.as_deref(),
            scope.tenant.as_deref(),
            scope.project.as_deref(),
            scope.repository.as_deref(),
        ]
        .iter()
        .filter_map(|s| s.map(String::from))
        .collect();
        md.push_str(&format!("**Scope:** {}\n\n", parts.join(" / ")));
    }

    if plan.replan_count > 0 {
        md.push_str(&format!("**Replans:** {}\n\n", plan.replan_count));
    }

    // Nodes table
    md.push_str("## Nodes\n\n");
    md.push_str("| Status | Label | Type | Cost |\n");
    md.push_str("|---|---|---|---|\n");

    let plan_suffix = plan.id.trim_start_matches("plan_");
    let mut total_cost = 0.0f64;
    for node in &plan.nodes {
        let cost = if let Some(u) = &node.token_usage {
            total_cost += u.estimated_cost_usd;
            format!("~${:.4}", u.estimated_cost_usd)
        } else {
            String::new()
        };
        md.push_str(&format!(
            "| {:?} | {} | {:?} | {} |\n",
            node.status, node.label, node.task_type, cost
        ));
    }
    if total_cost > 0.0 {
        md.push_str(&format!("\n**Total estimated cost:** ~${total_cost:.4}\n\n"));
    }

    // Node output logs
    let has_logs = plan.nodes.iter().any(|n| {
        matches!(n.status, orbit_core::plan::NodeStatus::Completed | orbit_core::plan::NodeStatus::Failed | orbit_core::plan::NodeStatus::Running)
    });
    if has_logs {
        md.push_str("## Node Output\n\n");
        for node in &plan.nodes {
            if !matches!(node.status, orbit_core::plan::NodeStatus::Completed | orbit_core::plan::NodeStatus::Failed | orbit_core::plan::NodeStatus::Running) {
                continue;
            }
            let session_key = format!("orbit-plan-{plan_suffix}-{}", node.id);
            if let Some(log) = node_log_preview(&session_key, 20) {
                md.push_str(&format!("### {} ({:?})\n\n", node.label, node.status));
                md.push_str("```\n");
                md.push_str(&log);
                md.push_str("\n```\n\n");
            }
        }
    }

    // Audit trail
    if !audit_trail.is_empty() {
        md.push_str("## Audit Trail\n\n");
        for ev in audit_trail {
            md.push_str(&format!("- `{ev:?}`\n"));
        }
        md.push('\n');
    }

    // Memory record
    if let Some(run) = memory_run {
        md.push_str("## Memory Record\n\n");
        md.push_str(&format!("- **Outcome:** {}\n", run.outcome));
        md.push_str(&format!("- **Nodes:** {}\n", run.node_count));
        md.push_str(&format!("- **Replans:** {}\n", run.replan_count));
        md.push_str(&format!("- **Duration:** {}s\n\n", run.duration_secs));
    }

    md
}

/// Returns last `n` lines from a node's captured log, or None if no log exists.
fn node_log_preview(session_key: &str, n: usize) -> Option<String> {
    let log_path = std::env::temp_dir()
        .join("orbit-plan-nodes")
        .join(format!("{session_key}.log"));
    let content = std::fs::read_to_string(log_path).ok()?;
    if content.is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(n);
    Some(lines[start..].join("\n"))
}

fn resolve_scope_from_cwd() -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    // Prefer git repo root over cwd — handles subdirectory invocations correctly.
    let anchor = git_repo_root().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_default();

    let parts: Vec<String> = anchor
        .strip_prefix(&home)
        .ok()
        .map(|p| {
            p.components()
                .filter_map(|c| c.as_os_str().to_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    match parts.as_slice() {
        [ws, tenant, project, repo, ..] => (
            Some(ws.clone()),
            Some(tenant.clone()),
            Some(project.clone()),
            Some(repo.clone()),
        ),
        [ws, tenant, project] => {
            (Some(ws.clone()), Some(tenant.clone()), Some(project.clone()), None)
        }
        [ws, tenant] => (Some(ws.clone()), Some(tenant.clone()), None, None),
        [ws] => (Some(ws.clone()), None, None, None),
        _ => (None, None, None, None),
    }
}

/// Returns the git repository root for the current directory, if inside a git repo.
fn git_repo_root() -> Option<std::path::PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?;
        Some(std::path::PathBuf::from(path.trim()))
    } else {
        None
    }
}

// ── template subcommands ──────────────────────────────────────────────────────

async fn run_template(command: TemplateCommand) -> Result<()> {
    match command {
        TemplateCommand::List => {
            let templates = template::list_templates();
            if templates.is_empty() {
                let dir = template::templates_dir();
                println!("No templates found.");
                println!("Create one with: orbit plan template create <name>");
                println!("Templates dir:   {}", dir.display());
                return Ok(());
            }
            println!("{} template(s) in {}:", templates.len(), template::templates_dir().display());
            println!();
            for t in &templates {
                let vars = t.variables();
                let var_hint = if vars.is_empty() {
                    String::new()
                } else {
                    format!("  [vars: {}]", vars.join(", "))
                };
                println!("  {}  —  {}{}", t.name, t.description, var_hint);
            }
            println!();
            println!("Run with: orbit plan template run <name> [key=value ...]");
        }

        TemplateCommand::Show { name } => {
            let t = template::load_template(&name)?;
            println!("Template: {}", t.name);
            println!("Description: {}", t.description);
            println!("Intent: {}", t.intent);
            let vars = t.variables();
            if !vars.is_empty() {
                println!("Variables: {}", vars.join(", "));
            }
            if !t.repos.is_empty() {
                println!("Default repos: {}", t.repos.join(", "));
            }
            println!();
            println!("Run with: orbit plan template run {} {}", t.name,
                vars.iter().map(|v| format!("{v}=<value>")).collect::<Vec<_>>().join(" "));
            println!("File: {}", template::template_path(&name).display());
        }

        TemplateCommand::Run {
            name,
            vars,
            dry_run,
            foreground,
            workspace,
            tenant,
            project,
            repository,
        } => {
            let t = template::load_template(&name)?;
            let var_map = template::parse_vars(&vars)?;
            let intent = t.render(&var_map)?;

            let (workspace, tenant, project, repository) =
                if workspace.is_none() && tenant.is_none() && project.is_none() && repository.is_none() {
                    resolve_scope_from_cwd()
                } else {
                    (workspace, tenant, project, repository)
                };

            let extra_repos: Vec<CrossRepoSpec> = t
                .repos
                .iter()
                .map(|p| {
                    let path = std::path::Path::new(p);
                    let alias = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(p)
                        .to_string();
                    CrossRepoSpec {
                        alias: alias.clone(),
                        workspace: None,
                        tenant: None,
                        project: None,
                        repository: Some(alias),
                    }
                })
                .collect();

            println!("Template: {}", t.name);
            println!("Planning: {intent}");
            if dry_run {
                println!("(dry-run — plan will not execute)");
            }

            match send_raw(&Request::CreatePlan {
                intent: intent.clone(),
                workspace,
                tenant,
                project,
                repository,
                dry_run,
                verbose: false,
                extra_repos,
            })
            .await?
            {
                Response::PlanCreated { id, node_count, trace: _ } => {
                    println!("Plan created: {id} ({node_count} node(s))");
                    if !dry_run {
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let orbit_dir = cwd.join(".orbit");
                        if orbit_dir.exists() || std::fs::create_dir_all(&orbit_dir).is_ok() {
                            let sock_path = orbit_dir.join("orbit.sock");
                            let _ = send_raw(&Request::AddProjectSocket {
                                path: sock_path.to_string_lossy().into_owned(),
                                role: ProjectRole::Contributor,
                            })
                            .await;
                        }
                        if foreground {
                            stream_until_done(&id).await;
                        } else {
                            println!("Running. Check status with: orbit plan get {id}");
                            println!("Stream live output with:    orbit plan watch {id}");
                        }
                    }
                }
                Response::Error { message } => {
                    eprintln!("Error: {message}");
                    std::process::exit(1);
                }
                _ => eprintln!("Unexpected response"),
            }
        }

        TemplateCommand::Create { name } => {
            let path = template::template_path(&name);
            if path.exists() {
                eprintln!("Template '{}' already exists: {}", name, path.display());
                eprintln!("Edit it with: $EDITOR {}", path.display());
                std::process::exit(1);
            }
            std::fs::create_dir_all(template::templates_dir())?;
            std::fs::write(&path, template::starter_toml(&name))?;

            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = std::process::Command::new(&editor)
                .arg(&path)
                .status()?;
            if status.success() {
                println!("Template saved: {}", path.display());
                println!("Run it with:    orbit plan template run {name}");
            } else {
                eprintln!("Editor exited with non-zero status. Template file kept at: {}", path.display());
            }
        }

        TemplateCommand::FromPlan { plan_id, name, description } => {
            let plan = Plan::load(&plan_id)
                .map_err(|e| anyhow::anyhow!("plan not found: {e}"))?;
            let desc = if description.is_empty() {
                format!("Captured from plan {}", plan_id)
            } else {
                description
            };
            let t = orbit_core::template::PlanTemplate {
                name: name.clone(),
                description: desc,
                intent: plan.intent.clone(),
                repos: vec![],
            };
            template::save_template(&name, &t)?;
            println!("Template '{}' saved.", name);
            println!("Intent: {}", plan.intent);
            println!("File:   {}", template::template_path(&name).display());
            println!("Edit:   orbit plan template show {name}");
        }
    }
    Ok(())
}
