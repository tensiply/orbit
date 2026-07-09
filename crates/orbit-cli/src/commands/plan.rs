use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_client::ipc::send_raw;
use orbit_core::{
    audit::events_for_plan,
    eval::EvalConstraint,
    ipc::{PlanStreamEvent, Request, Response},
    memory::find_run,
    plan::{Plan, PlanNodeType},
};
use serde::Serialize;

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
                        println!(
                            "  [{:?}] {} — {:?}{}",
                            node.status, node.label, node.task_type, cost_str
                        );
                        if node.status == orbit_core::plan::NodeStatus::Running {
                            let session_key = format!("orbit-plan-{plan_suffix}-{}", node.id);
                            println!("         tmux attach -t {session_key}");
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

        Some(PlanCommand::Export { id, stdout }) => {
            let plan = Plan::load(&id).map_err(|e| anyhow::anyhow!("plan not found: {e}"))?;
            let audit_trail = events_for_plan(&id);
            let memory_run = find_run(&id);

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

fn resolve_scope_from_cwd() -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_default();

    let parts: Vec<String> = cwd
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
