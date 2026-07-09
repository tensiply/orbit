use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_client::ipc::send_raw;
use orbit_core::ipc::{Request, Response};

#[derive(Debug, Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub command: Option<PlanCommand>,

    /// Intent for plan creation (used when no subcommand given)
    pub intent: Option<String>,

    /// Preview the plan without executing it
    #[arg(long)]
    pub dry_run: bool,

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
}

pub async fn run(args: PlanArgs) -> Result<()> {
    match args.command {
        Some(PlanCommand::Get { id }) => {
            match send_raw(&Request::GetPlan { id }).await? {
                Response::PlanInfo { plan } => {
                    println!("Plan:   {} [{:?}]", plan.id, plan.status);
                    println!("Intent: {}", plan.intent);
                    println!("Nodes ({}):", plan.nodes.len());
                    for node in &plan.nodes {
                        println!("  [{:?}] {} — {:?}", node.status, node.label, node.task_type);
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
                    }
                    for plan in &plans {
                        println!(
                            "{} [{:?}] {} node(s) — {}",
                            plan.id,
                            plan.status,
                            plan.nodes.len(),
                            plan.intent
                        );
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
            })
            .await?
            {
                Response::PlanCreated { id, node_count } => {
                    println!("Plan created: {id} ({node_count} node(s))");
                    if !args.dry_run {
                        println!("Running. Check status with: orbit plan get {id}");
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
