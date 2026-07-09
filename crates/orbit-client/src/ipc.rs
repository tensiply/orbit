use anyhow::{Result, bail};
use orbit_core::{
    ipc::{PlanStreamEvent, Request, Response, socket_path},
    session::Session,
};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

// ── client ────────────────────────────────────────────────────────────────────

/// Returns `true` if the daemon socket exists (daemon may be running).
pub fn is_available() -> bool {
    socket_path().exists()
}

pub async fn send_raw(req: &Request) -> Result<Response> {
    send(req).await
}

async fn send(req: &Request) -> Result<Response> {
    let sock = socket_path();
    if !sock.exists() {
        bail!("Daemon is not running. Start it with `orbit daemon start`.");
    }

    let stream = UnixStream::connect(&sock).await?;
    let (reader, mut writer) = stream.into_split();

    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;

    let mut resp_line = String::new();
    BufReader::new(reader).read_line(&mut resp_line).await?;

    Ok(serde_json::from_str(resp_line.trim())?)
}

// ── convenience methods ───────────────────────────────────────────────────────

pub async fn list_sessions() -> Result<Vec<Session>> {
    match send(&Request::ListSessions).await? {
        Response::Sessions { sessions } => Ok(sessions),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn kill_session(id: &str) -> Result<()> {
    match send(&Request::KillSession { id: id.to_string() }).await? {
        Response::Killed { .. } => Ok(()),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn clean_sessions() -> Result<usize> {
    match send(&Request::CleanSessions).await? {
        Response::Cleaned { count } => Ok(count),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub struct StatusInfo {
    pub uptime_secs: u64,
    pub session_count: usize,
    pub pid: u32,
}

pub async fn status() -> Result<StatusInfo> {
    match send(&Request::Status).await? {
        Response::Status {
            uptime_secs,
            session_count,
            pid,
        } => Ok(StatusInfo {
            uptime_secs,
            session_count,
            pid,
        }),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn list_plans() -> Result<Vec<orbit_core::plan::Plan>> {
    match send(&Request::ListPlans).await? {
        Response::Plans { plans } => Ok(plans),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn cancel_plan(id: &str) -> Result<()> {
    match send(&Request::CancelPlan { id: id.to_string() }).await? {
        Response::PlanCancelled { .. } => Ok(()),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn shutdown() -> Result<()> {
    match send(&Request::Shutdown).await? {
        Response::Ok => Ok(()),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub struct LaunchedInfo {
    pub tmux_name: String,
    pub session_id: String,
}

/// Subscribe to live events for a running plan.
/// Returns a channel receiver — events arrive until the plan reaches a terminal state.
pub async fn stream_plan(id: &str) -> Result<tokio::sync::mpsc::Receiver<PlanStreamEvent>> {
    stream_plan_on(id, socket_path()).await
}

/// Like `stream_plan` but connects to a specific socket path (e.g. a project socket).
pub async fn stream_plan_on(id: &str, sock: PathBuf) -> Result<tokio::sync::mpsc::Receiver<PlanStreamEvent>> {
    if !sock.exists() {
        bail!("Daemon is not running. Start it with `orbit daemon start`.");
    }

    let stream = UnixStream::connect(&sock).await?;
    let (reader, mut writer) = stream.into_split();

    let req = Request::StreamPlan { id: id.to_string() };
    let mut line = serde_json::to_string(&req)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;

    let (tx, rx) = tokio::sync::mpsc::channel::<PlanStreamEvent>(64);

    tokio::spawn(async move {
        let _writer = writer; // keep connection alive
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match serde_json::from_str::<PlanStreamEvent>(&line) {
                Ok(event) => {
                    let terminal = event.is_terminal();
                    let _ = tx.send(event).await;
                    if terminal {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok(rx)
}

pub async fn launch_session(
    workspace: Option<String>,
    tenant: Option<String>,
    project: Option<String>,
    repository: Option<String>,
    engine: &str,
    no_tmux: bool,
) -> Result<LaunchedInfo> {
    match send(&Request::LaunchSession {
        workspace,
        tenant,
        project,
        repository,
        engine: engine.to_string(),
        no_tmux,
    })
    .await?
    {
        Response::Launched {
            tmux_name,
            session_id,
        } => Ok(LaunchedInfo {
            tmux_name,
            session_id,
        }),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}
