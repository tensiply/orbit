use anyhow::Result;
use orbit_core::{
    audit::audit_stats,
    ipc::{PlanStreamEvent, Request, Response, pid_path, socket_path},
    plan::{NodeStatus, Plan, PlanStatus},
    session::Session,
};
use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::broadcast,
};
use tracing::{debug, info, warn};

// ── connection role ───────────────────────────────────────────────────────────

/// Restricts which requests a connection may issue.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionRole {
    /// Full access (owner Unix socket).
    Owner,
    /// Read + approve AwaitingApproval nodes (project socket, contributor role).
    Contributor,
    /// Read-only; cannot approve nodes (project socket, observer role).
    Observer,
}

impl ConnectionRole {
    fn allows(&self, req: &Request) -> bool {
        match self {
            ConnectionRole::Owner => true,
            ConnectionRole::Contributor => matches!(
                req,
                Request::GetPlan { .. }
                    | Request::ListPlans
                    | Request::GetPlanStats
                    | Request::ApprovePlanNode { .. }
                    | Request::PausePlan { .. }
                    | Request::ResumePlan { .. }
                    | Request::StreamPlan { .. }
                    | Request::ListSchedules
            ),
            ConnectionRole::Observer => matches!(
                req,
                Request::GetPlan { .. }
                    | Request::ListPlans
                    | Request::GetPlanStats
                    | Request::StreamPlan { .. }
                    | Request::ListSchedules
            ),
        }
    }
}

// ── state ─────────────────────────────────────────────────────────────────────

struct ServerState {
    started_at: Instant,
    shutdown_tx: broadcast::Sender<()>,
    event_tx: broadcast::Sender<PlanStreamEvent>,
}

impl ServerState {
    fn new(shutdown_tx: broadcast::Sender<()>, event_tx: broadcast::Sender<PlanStreamEvent>) -> Arc<Self> {
        Arc::new(Self {
            started_at: Instant::now(),
            shutdown_tx,
            event_tx,
        })
    }

    fn handle(&self, req: Request, role: ConnectionRole) -> Response {
        if !role.allows(&req) {
            return Response::Error {
                message: "operation not permitted on project socket".into(),
            };
        }
        match req {
            Request::ListSessions => {
                let sessions = Session::load_all();
                Response::Sessions { sessions }
            }

            Request::KillSession { id } => {
                let sessions = Session::load_all();
                match sessions.into_iter().find(|s| s.id == id) {
                    None => Response::Error {
                        message: format!("Session {id} not found"),
                    },
                    Some(session) => {
                        send_sigterm(session.pid);
                        let _ = session.delete();
                        Response::Killed { id }
                    }
                }
            }

            Request::CleanSessions => {
                let sessions = Session::load_all();
                let mut count = 0usize;
                for s in &sessions {
                    if !s.is_running() {
                        let _ = s.delete();
                        count += 1;
                    }
                }
                Response::Cleaned { count }
            }

            Request::Status => {
                let sessions = Session::load_all();
                let alive = sessions.iter().filter(|s| s.is_running()).count();
                Response::Status {
                    uptime_secs: self.started_at.elapsed().as_secs(),
                    session_count: alive,
                    pid: std::process::id(),
                }
            }

            Request::Shutdown => {
                let _ = self.shutdown_tx.send(());
                Response::Ok
            }

            Request::LaunchSession {
                workspace,
                tenant,
                project,
                repository,
                engine,
                no_tmux,
            } => {
                use orbit_core::engine::Engine;
                use orbit_engine::{
                    config, launcher,
                    resolver::{self, ResolveArgs},
                };

                if no_tmux {
                    return Response::Error {
                        message: "daemon cannot launch without tmux — would replace daemon process"
                            .into(),
                    };
                }

                let engine_val = match engine.as_str() {
                    "opencode" => Engine::Opencode,
                    "gemini" => Engine::Gemini,
                    "claude" => Engine::Claude,
                    other => {
                        return Response::Error {
                            message: format!("unknown engine: {other}"),
                        };
                    }
                };

                let scope = match resolver::resolve(ResolveArgs {
                    workspace,
                    tenant,
                    project,
                    repository,
                }) {
                    Ok(s) => s,
                    Err(e) => {
                        return Response::Error {
                            message: e.to_string(),
                        };
                    }
                };

                let merged = match config::load(&scope, engine_val) {
                    Ok(m) => m,
                    Err(e) => {
                        return Response::Error {
                            message: e.to_string(),
                        };
                    }
                };

                match launcher::spawn_background(&scope, &merged, engine_val, None, None) {
                    Ok(session) => Response::Launched {
                        tmux_name: session.tmux_session.unwrap_or_default(),
                        session_id: session.id,
                    },
                    Err(e) => Response::Error {
                        message: e.to_string(),
                    },
                }
            }

            // ── Plan requests ─────────────────────────────────────────────────

            Request::CreatePlan {
                intent,
                workspace,
                tenant,
                project,
                repository,
                dry_run,
                verbose,
                extra_repos,
            } => {
                use orbit_core::{
                    audit::{append_event, AuditEvent},
                    memory::{find_similar, load_recent_runs},
                    plan::{PlanScope, PlanStatus},
                };
                use orbit_engine::resolver::{self, ResolveArgs};
                use orbit_planner::{backend::CliBackend, planner::{PlannerConfig, invoke_planner}};

                let scope = PlanScope { workspace, tenant, project, repository };
                // Prefer semantically relevant past runs over purely chronological ones.
                let recent = {
                    let similar = find_similar(&intent, 5);
                    if similar.is_empty() { load_recent_runs(3) } else { similar }
                };
                let cfg = PlannerConfig::default();

                match invoke_planner(&intent, &scope, &recent, &cfg, &CliBackend::new(cfg.engine), &extra_repos) {
                    Err(e) => Response::Error {
                        message: format!("planner error: {e}"),
                    },
                    Ok((mut plan, trace)) => {
                        // Validate all scope_overrides resolve
                        for node in &plan.nodes {
                            if let Some(ref s) = node.scope_override {
                                if let Err(e) = resolver::resolve(ResolveArgs {
                                    workspace: s.workspace.clone(),
                                    tenant: s.tenant.clone(),
                                    project: s.project.clone(),
                                    repository: s.repository.clone(),
                                }) {
                                    return Response::Error {
                                        message: format!("node {} scope_override invalid: {e}", node.id),
                                    };
                                }
                            }
                        }

                        let node_count = plan.nodes.len();
                        let trace_out = if verbose { Some(trace) } else { None };
                        if dry_run {
                            plan.status = PlanStatus::Planning;
                            Response::PlanCreated {
                                id: plan.id,
                                node_count,
                                trace: trace_out,
                            }
                        } else {
                            plan.status = PlanStatus::Running;
                            let _ = append_event(&AuditEvent::PlanCreated {
                                plan_id: plan.id.clone(),
                                intent: intent.clone(),
                                node_count,
                                timestamp: now_secs(),
                            });
                            orbit_core::hooks::run_hooks(
                                &orbit_core::hooks::HookEvent::PrePlan,
                                &[("ORBIT_PLAN_ID", &plan.id), ("ORBIT_PLAN_INTENT", &intent)],
                            );
                            match plan.save() {
                                Ok(_) => {
                                    orbit_core::hooks::run_hooks(
                                        &orbit_core::hooks::HookEvent::OnPlanCreated,
                                        &[("ORBIT_PLAN_ID", &plan.id), ("ORBIT_PLAN_INTENT", &intent)],
                                    );
                                    Response::PlanCreated {
                                        id: plan.id,
                                        node_count,
                                        trace: trace_out,
                                    }
                                }
                                Err(e) => Response::Error {
                                    message: format!("save error: {e}"),
                                },
                            }
                        }
                    }
                }
            }

            Request::GetPlan { id } => match orbit_core::plan::Plan::load(&id) {
                Ok(plan) => Response::PlanInfo { plan },
                Err(e) => Response::Error {
                    message: format!("plan not found: {e}"),
                },
            },

            Request::ListPlans => {
                let plans = orbit_core::plan::Plan::load_all();
                Response::Plans { plans }
            }

            Request::CancelPlan { id } => match orbit_core::plan::Plan::load(&id) {
                Err(e) => Response::Error {
                    message: format!("plan not found: {e}"),
                },
                Ok(mut plan) => {
                    plan.status = orbit_core::plan::PlanStatus::Cancelled;
                    let _ = plan.save();
                    Response::PlanCancelled { id }
                }
            },

            Request::ApprovePlanNode { plan_id, node_id } => {
                match orbit_core::plan::Plan::load(&plan_id) {
                    Err(e) => Response::Error {
                        message: format!("plan not found: {e}"),
                    },
                    Ok(mut plan) => {
                        match plan.nodes.iter_mut().find(|n| n.id == node_id) {
                            None => Response::Error {
                                message: format!("node {node_id} not found in plan {plan_id}"),
                            },
                            Some(node) if node.status != NodeStatus::AwaitingApproval => {
                                Response::Error {
                                    message: format!(
                                        "node {node_id} is not awaiting approval (status: {:?})",
                                        node.status
                                    ),
                                }
                            }
                            Some(node) => {
                                node.status = NodeStatus::Pending;
                                match plan.save() {
                                    Ok(_) => {
                                        info!("node {node_id} approved in plan {plan_id}");
                                        Response::PlanApproved { plan_id, node_id }
                                    }
                                    Err(e) => Response::Error {
                                        message: format!("save error: {e}"),
                                    },
                                }
                            }
                        }
                    }
                }
            }

            Request::GetPlanStats => {
                let stats = audit_stats();
                Response::PlanStats { stats }
            }

            Request::RetryPlan { id } => match orbit_core::plan::Plan::load(&id) {
                Err(e) => Response::Error {
                    message: format!("plan not found: {e}"),
                },
                Ok(mut plan) => {
                    use orbit_core::plan::PlanStatus;
                    match plan.status {
                        PlanStatus::Running | PlanStatus::Planning => Response::Error {
                            message: format!("plan {id} is still running — cancel it first"),
                        },
                        PlanStatus::Completed => Response::Error {
                            message: format!("plan {id} completed successfully — nothing to retry"),
                        },
                        _ => {
                            let mut reset_count = 0usize;
                            for node in plan.nodes.iter_mut() {
                                if node.status == NodeStatus::Failed {
                                    node.status = NodeStatus::Pending;
                                    node.error = None;
                                    node.output_summary = None;
                                    node.session_id = None;
                                    node.started_at = None;
                                    node.completed_at = None;
                                    node.retry_count = 0;
                                    reset_count += 1;
                                }
                            }
                            if reset_count == 0 {
                                return Response::Error {
                                    message: format!("plan {id} has no failed nodes to retry"),
                                };
                            }
                            plan.status = PlanStatus::Running;
                            match plan.save() {
                                Ok(_) => {
                                    info!("plan {id} retried: {reset_count} node(s) reset to Pending");
                                    Response::PlanRetried { id, reset_count }
                                }
                                Err(e) => Response::Error {
                                    message: format!("save error: {e}"),
                                },
                            }
                        }
                    }
                }
            },

            Request::PausePlan { id } => match Plan::load(&id) {
                Err(e) => Response::Error { message: format!("plan not found: {e}") },
                Ok(mut plan) => {
                    if plan.status != PlanStatus::Running {
                        Response::Error {
                            message: format!("plan {id} is not Running (status: {:?})", plan.status),
                        }
                    } else {
                        plan.status = PlanStatus::Paused;
                        match plan.save() {
                            Ok(_) => {
                                info!("plan {id} paused");
                                Response::PlanPaused { id }
                            }
                            Err(e) => Response::Error { message: format!("save error: {e}") },
                        }
                    }
                }
            },

            Request::ResumePlan { id } => match Plan::load(&id) {
                Err(e) => Response::Error { message: format!("plan not found: {e}") },
                Ok(mut plan) => {
                    if plan.status != PlanStatus::Paused {
                        Response::Error {
                            message: format!("plan {id} is not Paused (status: {:?})", plan.status),
                        }
                    } else {
                        plan.status = PlanStatus::Running;
                        match plan.save() {
                            Ok(_) => {
                                info!("plan {id} resumed");
                                Response::PlanResumed { id }
                            }
                            Err(e) => Response::Error { message: format!("save error: {e}") },
                        }
                    }
                }
            },

            // StreamPlan is handled in handle_connection before reaching here.
            Request::StreamPlan { .. } => Response::Error {
                message: "StreamPlan must be the first request on a connection".into(),
            },

            Request::AddProjectSocket { path, role } => {
                use orbit_core::ipc::ProjectRole;
                let conn_role = match role {
                    ProjectRole::Contributor => ConnectionRole::Contributor,
                    ProjectRole::Observer => ConnectionRole::Observer,
                };
                let path_buf = std::path::PathBuf::from(&path);
                if let Some(parent) = path_buf.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if path_buf.exists() {
                    let _ = fs::remove_file(&path_buf);
                }
                match UnixListener::bind(&path_buf) {
                    Err(e) => Response::Error {
                        message: format!("bind error: {e}"),
                    },
                    Ok(listener) => {
                        info!("project socket ({role:?}) bound at {path}");
                        let state = Arc::new(ServerState {
                            started_at: self.started_at,
                            shutdown_tx: self.shutdown_tx.clone(),
                            event_tx: self.event_tx.clone(),
                        });
                        let mut shutdown_rx = self.shutdown_tx.subscribe();
                        tokio::spawn(async move {
                            loop {
                                tokio::select! {
                                    accept = listener.accept() => {
                                        match accept {
                                            Ok((stream, _)) => {
                                                let s = state.clone();
                                                tokio::spawn(handle_connection(stream, s, conn_role));
                                            }
                                            Err(e) => { warn!("project socket accept error: {e}"); break; }
                                        }
                                    }
                                    _ = shutdown_rx.recv() => {
                                        let _ = fs::remove_file(&path_buf);
                                        break;
                                    }
                                }
                            }
                        });
                        Response::ProjectSocketAdded { path }
                    }
                }
            }

            Request::EvalPlan {
                intent,
                workspace,
                tenant,
                project,
                repository,
                constraints,
            } => {
                use orbit_core::{memory::load_recent_runs, plan::PlanScope};
                use orbit_planner::{backend::CliBackend, planner::{PlannerConfig, invoke_planner}};

                let scope = PlanScope { workspace, tenant, project, repository };
                let recent = load_recent_runs(5);
                let cfg = PlannerConfig::default();

                match invoke_planner(&intent, &scope, &recent, &cfg, &CliBackend::new(cfg.engine), &[]) {
                    Err(e) => Response::Error {
                        message: format!("planner error: {e}"),
                    },
                    Ok((plan, _trace)) => {
                        let result = orbit_eval::eval(&plan, &constraints);
                        Response::PlanEvalResult { plan, result }
                    }
                }
            }

            Request::CreateSchedule { intent, at, cron, repos, workspace, tenant, project, repository } => {
                use orbit_core::{
                    schedule::{ScheduleKind, ScheduledPlan, new_id, next_cron_after, now_secs, upsert},
                };

                let kind = match (at, cron) {
                    (Some(ts), _) => ScheduleKind::Once { at: ts },
                    (None, Some(expr)) => ScheduleKind::Cron { expr: expr.clone() },
                    (None, None) => return Response::Error {
                        message: "CreateSchedule requires either `at` (timestamp) or `cron` (expression)".into(),
                    },
                };

                let next_run = match &kind {
                    ScheduleKind::Once { at } => Some(*at),
                    ScheduleKind::Cron { expr } => {
                        match next_cron_after(expr, now_secs()) {
                            Ok(t) => t,
                            Err(e) => return Response::Error {
                                message: format!("invalid cron expression: {e}"),
                            },
                        }
                    }
                };

                let repos_paths = repos.iter().map(std::path::PathBuf::from).collect();
                let id = new_id();
                let sched = ScheduledPlan {
                    id: id.clone(),
                    intent,
                    schedule: kind,
                    repos: repos_paths,
                    workspace,
                    tenant,
                    project,
                    repository,
                    next_run,
                    last_run: None,
                    run_count: 0,
                    created_at: now_secs(),
                };

                match upsert(sched) {
                    Ok(_) => Response::ScheduleCreated { id, next_run },
                    Err(e) => Response::Error { message: format!("failed to save schedule: {e}") },
                }
            }

            Request::ListSchedules => {
                Response::Schedules { schedules: orbit_core::schedule::load_all() }
            }

            Request::CancelSchedule { id } => {
                match orbit_core::schedule::delete(&id) {
                    Ok(true) => Response::ScheduleCancelled { id },
                    Ok(false) => Response::Error { message: format!("schedule not found: {id}") },
                    Err(e) => Response::Error { message: format!("failed to delete schedule: {e}") },
                }
            }

            Request::RunScheduleNow { id } => {
                use orbit_core::{
                    audit::{append_event, AuditEvent},
                    memory::{find_similar, load_recent_runs},
                    plan::{PlanScope, PlanStatus},
                    schedule::{upsert, now_secs},
                };
                use orbit_planner::{backend::CliBackend, planner::{PlannerConfig, invoke_planner}};

                let sched = match orbit_core::schedule::find(&id) {
                    Some(s) => s,
                    None => return Response::Error { message: format!("schedule not found: {id}") },
                };

                let scope = PlanScope {
                    workspace: sched.workspace.clone(),
                    tenant: sched.tenant.clone(),
                    project: sched.project.clone(),
                    repository: sched.repository.clone(),
                };
                let extra_repos: Vec<_> = sched.repos.iter().map(|p| {
                    orbit_core::plan::CrossRepoSpec {
                        alias: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        workspace: None, tenant: None, project: None,
                        repository: Some(p.to_string_lossy().to_string()),
                    }
                }).collect();
                let recent = {
                    let similar = find_similar(&sched.intent, 5);
                    if similar.is_empty() { load_recent_runs(3) } else { similar }
                };
                let cfg = PlannerConfig::default();

                match invoke_planner(&sched.intent, &scope, &recent, &cfg, &CliBackend::new(cfg.engine), &extra_repos) {
                    Err(e) => Response::Error { message: format!("planner error: {e}") },
                    Ok((mut plan, _trace)) => {
                        plan.status = PlanStatus::Running;
                        let plan_id = plan.id.clone();
                        let _ = append_event(&AuditEvent::PlanCreated {
                            plan_id: plan_id.clone(),
                            intent: sched.intent.clone(),
                            node_count: plan.nodes.len(),
                            timestamp: now_secs(),
                        });
                        if let Err(e) = plan.save() {
                            return Response::Error { message: format!("failed to save plan: {e}") };
                        }
                        let mut updated = sched;
                        updated.last_run = Some(now_secs());
                        updated.run_count += 1;
                        let _ = upsert(updated);
                        Response::ScheduleFired { schedule_id: id, plan_id }
                    }
                }
            }
        }
    }
}

// ── connection handler ────────────────────────────────────────────────────────

async fn handle_connection(stream: UnixStream, state: Arc<ServerState>, role: ConnectionRole) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        debug!("ipc request: {line}");

        let req = match serde_json::from_str::<Request>(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = Response::Error { message: format!("parse error: {e}") };
                let mut json = serde_json::to_string(&err).unwrap_or_default();
                json.push('\n');
                let _ = writer.write_all(json.as_bytes()).await;
                break;
            }
        };

        // ── streaming path ────────────────────────────────────────────────────
        if let Request::StreamPlan { ref id } = req {
            if !role.allows(&req) {
                let err = Response::Error { message: "operation not permitted on project socket".into() };
                let mut json = serde_json::to_string(&err).unwrap_or_default();
                json.push('\n');
                let _ = writer.write_all(json.as_bytes()).await;
                break;
            }

            // Subscribe before checking current state to avoid missing events.
            let mut rx = state.event_tx.subscribe();

            // If already terminal, send the event immediately.
            let current_terminal = orbit_core::plan::Plan::load(id).ok().and_then(|p| {
                match p.status {
                    PlanStatus::Completed => Some(PlanStreamEvent::PlanCompleted { plan_id: id.clone() }),
                    PlanStatus::Failed => Some(PlanStreamEvent::PlanFailed { plan_id: id.clone() }),
                    _ => None,
                }
            });

            if let Some(ev) = current_terminal {
                let mut json = serde_json::to_string(&ev).unwrap_or_default();
                json.push('\n');
                let _ = writer.write_all(json.as_bytes()).await;
                break;
            }

            // Forward events for this plan until terminal.
            loop {
                match rx.recv().await {
                    Ok(event) if event.plan_id() == id.as_str() => {
                        let terminal = event.is_terminal();
                        let mut json = serde_json::to_string(&event).unwrap_or_default();
                        json.push('\n');
                        if writer.write_all(json.as_bytes()).await.is_err() {
                            break;
                        }
                        if terminal {
                            break;
                        }
                    }
                    Ok(_) => {} // different plan — skip
                    Err(broadcast::error::RecvError::Lagged(_)) => {} // drop and continue
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            break;
        }

        // ── regular request/response path ─────────────────────────────────────
        let response = state.handle(req, role);
        let mut json = serde_json::to_string(&response).unwrap_or_default();
        json.push('\n');
        if writer.write_all(json.as_bytes()).await.is_err() {
            break;
        }
    }
}

// ── auto-resume ───────────────────────────────────────────────────────────────

/// On daemon startup, re-attach output capture for any nodes still Running in tmux.
/// Nodes whose tmux session died will be handled by the next supervisor tick.
fn resume_running_plans() {
    use orbit_core::plan::Plan;

    let plans = Plan::load_all();
    let running_plans: Vec<&Plan> = plans.iter().filter(|p| p.status == PlanStatus::Running).collect();
    if running_plans.is_empty() {
        return;
    }

    info!("auto-resume: {} running plan(s) found at startup", running_plans.len());

    for plan in running_plans {
        for node in plan.nodes.iter().filter(|n| n.status == NodeStatus::Running) {
            let plan_suffix = plan.id.trim_start_matches("plan_");
            let session_key = format!("orbit-plan-{plan_suffix}-{}", node.id);

            let tmux_alive = std::process::Command::new("tmux")
                .args(["has-session", "-t", &session_key])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if tmux_alive {
                // Re-attach pipe-pane in case it was lost when the previous daemon died
                let log_dir = std::env::temp_dir().join("orbit-plan-nodes");
                let _ = std::fs::create_dir_all(&log_dir);
                let log_path = log_dir.join(format!("{session_key}.log"));
                let _ = std::process::Command::new("tmux")
                    .args([
                        "pipe-pane",
                        "-t",
                        &session_key,
                        &format!("cat >> {}", log_path.to_string_lossy()),
                    ])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                info!("auto-resume: re-attached output capture for node {} (plan {})", node.id, plan.id);
            } else {
                info!(
                    "auto-resume: tmux session gone for node {} (plan {}) — supervisor will handle",
                    node.id, plan.id
                );
            }
        }
    }
}

// ── server entry point ────────────────────────────────────────────────────────

pub async fn run() -> Result<()> {
    let sock = socket_path();
    let pid_file = pid_path();

    fs::create_dir_all(sock.parent().unwrap())?;

    // Remove stale socket file
    if sock.exists() {
        fs::remove_file(&sock)?;
    }

    // Write PID file
    fs::write(&pid_file, std::process::id().to_string())?;

    let listener = UnixListener::bind(&sock)?;
    info!("orbitd listening on {}", sock.display());

    resume_running_plans();

    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let (event_tx, _) = broadcast::channel::<PlanStreamEvent>(4096);
    let state = ServerState::new(shutdown_tx.clone(), event_tx.clone());

    // Background: auto-clean dead sessions every 60s
    tokio::spawn(crate::session_monitor::run_cleanup_loop(
        Duration::from_secs(60),
        shutdown_tx.subscribe(),
    ));

    // Background: advance running plans every 5s
    tokio::spawn(crate::plan_supervisor::run_supervisor_loop(
        Duration::from_secs(5),
        shutdown_tx.subscribe(),
        event_tx.clone(),
    ));

    // Background: check scheduled plans every 60s
    tokio::spawn(crate::scheduler::run_scheduler_loop(
        Duration::from_secs(60),
        shutdown_tx.subscribe(),
    ));

    // Background: poll Jira and write cache if plugin is installed
    let jira_installed = orbit_core::plugin::load_all()
        .iter()
        .any(|p| p.name == "jira" && p.is_installed());
    if jira_installed {
        let interval_secs = orbit_core::jira::poll_interval_secs();
        info!("jira poller enabled: every {interval_secs}s");
        tokio::spawn(crate::jira_poller::run_poll_loop(
            Duration::from_secs(interval_secs),
            shutdown_tx.subscribe(),
        ));
    }

    let mut shutdown_rx = shutdown_tx.subscribe();

    loop {
        tokio::select! {
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        tokio::spawn(handle_connection(stream, state, ConnectionRole::Owner));
                    }
                    Err(e) => warn!("accept error: {e}"),
                }
            }
            _ = shutdown_rx.recv() => {
                info!("shutdown requested — stopping orbitd");
                break;
            }
        }
    }

    // Cleanup
    let _ = fs::remove_file(&sock);
    let _ = fs::remove_file(&pid_file);

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn send_sigterm(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}
