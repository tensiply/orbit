use anyhow::Result;
use orbit_core::{
    ipc::{Request, Response, pid_path, socket_path},
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

// ── state ─────────────────────────────────────────────────────────────────────

struct ServerState {
    started_at: Instant,
    shutdown_tx: broadcast::Sender<()>,
}

impl ServerState {
    fn new(shutdown_tx: broadcast::Sender<()>) -> Arc<Self> {
        Arc::new(Self {
            started_at: Instant::now(),
            shutdown_tx,
        })
    }

    async fn handle(&self, req: Request) -> Response {
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

                match launcher::spawn_background(&scope, &merged, engine_val, None) {
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
            } => {
                use orbit_core::{
                    audit::{append_event, AuditEvent},
                    memory::load_recent_runs,
                    plan::{Plan, PlanScope, PlanStatus},
                };
                use orbit_planner::planner::{PlannerConfig, invoke_planner};

                let scope = PlanScope { workspace, tenant, project, repository };
                let recent = load_recent_runs(5);
                let cfg = PlannerConfig::default();

                match invoke_planner(&intent, &scope, &recent, &cfg) {
                    Err(e) => Response::Error {
                        message: format!("planner error: {e}"),
                    },
                    Ok(mut plan) => {
                        let node_count = plan.nodes.len();
                        if dry_run {
                            plan.status = PlanStatus::Planning;
                            Response::PlanCreated {
                                id: plan.id,
                                node_count,
                            }
                        } else {
                            plan.status = PlanStatus::Running;
                            let _ = append_event(&AuditEvent::PlanCreated {
                                plan_id: plan.id.clone(),
                                intent: intent.clone(),
                                node_count,
                                timestamp: now_secs(),
                            });
                            match plan.save() {
                                Ok(_) => Response::PlanCreated {
                                    id: plan.id,
                                    node_count,
                                },
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
        }
    }
}

// ── connection handler ────────────────────────────────────────────────────────

async fn handle_connection(stream: UnixStream, state: Arc<ServerState>) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        debug!("ipc request: {line}");
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => state.handle(req).await,
            Err(e) => Response::Error {
                message: format!("parse error: {e}"),
            },
        };

        let mut json = serde_json::to_string(&response).unwrap_or_default();
        json.push('\n');
        if writer.write_all(json.as_bytes()).await.is_err() {
            break;
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

    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let state = ServerState::new(shutdown_tx.clone());

    // Background: auto-clean dead sessions every 60s
    tokio::spawn(crate::session_monitor::run_cleanup_loop(
        Duration::from_secs(60),
        shutdown_tx.subscribe(),
    ));

    // Background: advance running plans every 5s
    tokio::spawn(crate::plan_supervisor::run_supervisor_loop(
        Duration::from_secs(5),
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
                        tokio::spawn(handle_connection(stream, state));
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
