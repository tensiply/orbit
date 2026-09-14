use anyhow::{Result, bail};
use interprocess::local_socket::{GenericFilePath, ToFsName, tokio::Stream, tokio::prelude::*};
use orbit_core::{
    ipc::{AttachFrame, PlanStreamEvent, Request, Response, endpoint_for_path, socket_endpoint},
    session::Session,
};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

// ── transport ───────────────────────────────────────────────────────────────
//
// The wire protocol (newline-delimited JSON) is platform-independent; only the
// local-socket transport differs. `interprocess` gives us a Unix domain socket
// on unix and a named pipe on Windows behind one `Stream` type.

async fn connect(endpoint: &str) -> std::io::Result<Stream> {
    let name = endpoint.to_fs_name::<GenericFilePath>()?;
    Stream::connect(name).await
}

// ── client ────────────────────────────────────────────────────────────────────

/// Returns `true` if the daemon appears to be running.
#[cfg(unix)]
pub fn is_available() -> bool {
    orbit_core::ipc::socket_path().exists()
}

/// Returns `true` if the daemon appears to be running (named-pipe connect probe).
#[cfg(windows)]
pub fn is_available() -> bool {
    use interprocess::local_socket::{GenericFilePath, Stream as SyncStream, ToFsName, prelude::*};
    socket_endpoint()
        .to_fs_name::<GenericFilePath>()
        .and_then(SyncStream::connect)
        .is_ok()
}

pub async fn send_raw(req: &Request) -> Result<Response> {
    send_on(&socket_endpoint(), req).await
}

/// Send a request to a daemon socket at an explicit path (used by integration tests).
pub async fn send_raw_to(sock: &std::path::Path, req: &Request) -> Result<Response> {
    send_on(&endpoint_for_path(sock), req).await
}

async fn send(req: &Request) -> Result<Response> {
    send_on(&socket_endpoint(), req).await
}

async fn send_on(endpoint: &str, req: &Request) -> Result<Response> {
    let stream = match connect(endpoint).await {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // Stale unix socket — remove it so the daemon can rebind after restart.
            #[cfg(unix)]
            let _ = std::fs::remove_file(endpoint);
            bail!("Daemon is not running (stale socket removed).");
        }
        Err(_) => bail!("Daemon is not running."),
    };
    let (reader, mut writer) = tokio::io::split(stream);

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
    pub channel: String,
    pub home: String,
}

pub async fn status() -> Result<StatusInfo> {
    match send(&Request::Status).await? {
        Response::Status {
            uptime_secs,
            session_count,
            pid,
            channel,
            home,
        } => Ok(StatusInfo {
            uptime_secs,
            session_count,
            pid,
            channel,
            home,
        }),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub struct HealthInfo {
    pub uptime_secs: u64,
    pub pid: u32,
    pub running_plans: usize,
    pub completed_today: usize,
    pub failed_today: usize,
    pub plan_files: usize,
    pub archived_plans: usize,
    pub memory_records: usize,
    pub auto_prune_enabled: bool,
    pub auto_prune_days: u32,
}

pub async fn health() -> Result<HealthInfo> {
    match send(&Request::Health).await? {
        Response::Health {
            uptime_secs,
            pid,
            running_plans,
            completed_today,
            failed_today,
            plan_files,
            archived_plans,
            memory_records,
            auto_prune_enabled,
            auto_prune_days,
        } => Ok(HealthInfo {
            uptime_secs,
            pid,
            running_plans,
            completed_today,
            failed_today,
            plan_files,
            archived_plans,
            memory_records,
            auto_prune_enabled,
            auto_prune_days,
        }),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn list_plans() -> Result<Vec<orbit_core::plan::Plan>> {
    list_plans_filtered(None).await
}

pub async fn list_plans_filtered(
    workspace_filter: Option<&str>,
) -> Result<Vec<orbit_core::plan::Plan>> {
    match send(&Request::ListPlans {
        workspace_filter: workspace_filter.map(|s| s.to_string()),
    })
    .await?
    {
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
    stream_plan_ep(id, socket_endpoint()).await
}

/// Like `stream_plan` but connects to a specific socket path (e.g. a project socket).
pub async fn stream_plan_on(
    id: &str,
    sock: PathBuf,
) -> Result<tokio::sync::mpsc::Receiver<PlanStreamEvent>> {
    stream_plan_ep(id, endpoint_for_path(&sock)).await
}

async fn stream_plan_ep(
    id: &str,
    endpoint: String,
) -> Result<tokio::sync::mpsc::Receiver<PlanStreamEvent>> {
    let stream = connect(&endpoint).await.map_err(|_| {
        anyhow::anyhow!("Daemon is not running. Start it with `orbit daemon start`.")
    })?;
    let (reader, mut writer) = tokio::io::split(stream);

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

// ── session attach ──────────────────────────────────────────────────────────
//
// A bidirectional PTY stream to a daemon-owned session. After the initial
// `SessionAttach`, both directions carry newline-JSON `AttachFrame`s. This is
// the transport primitive — terminal raw-mode and stdin/stdout pumping live in
// the CLI so this stays testable without a TTY.

pub struct AttachChannel {
    reader: tokio::io::Lines<BufReader<ReadHalf<Stream>>>,
    writer: WriteHalf<Stream>,
}

impl AttachChannel {
    /// Next chunk of PTY output, or `None` when the session ended (daemon closed
    /// the stream). Non-output frames are skipped.
    pub async fn recv_output(&mut self) -> Result<Option<Vec<u8>>> {
        loop {
            match self.reader.next_line().await? {
                Some(line) => match serde_json::from_str::<AttachFrame>(&line) {
                    Ok(AttachFrame::Output { bytes }) => return Ok(Some(bytes)),
                    Ok(_) => continue, // client-bound frame echoed back — ignore
                    Err(e) => bail!("malformed attach frame: {e}"),
                },
                None => return Ok(None),
            }
        }
    }

    /// Send client input (keystrokes / stdin) to the PTY.
    pub async fn send_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.send_frame(&AttachFrame::Input {
            bytes: bytes.to_vec(),
        })
        .await
    }

    /// Tell the daemon the terminal was resized.
    pub async fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.send_frame(&AttachFrame::Resize { cols, rows }).await
    }

    /// Detach cleanly (the daemon keeps the PTY alive).
    pub async fn detach(&mut self) -> Result<()> {
        self.send_frame(&AttachFrame::Detach).await
    }

    async fn send_frame(&mut self, frame: &AttachFrame) -> Result<()> {
        let mut line = serde_json::to_string(frame)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// Open an attach stream to a daemon-owned session's PTY.
pub async fn open_attach(id: &str, cols: u16, rows: u16) -> Result<AttachChannel> {
    open_attach_on(&socket_endpoint(), id, cols, rows).await
}

/// Like `open_attach` but connects to an explicit socket path (integration tests).
pub async fn open_attach_to(
    sock: &std::path::Path,
    id: &str,
    cols: u16,
    rows: u16,
) -> Result<AttachChannel> {
    open_attach_on(&endpoint_for_path(sock), id, cols, rows).await
}

async fn open_attach_on(endpoint: &str, id: &str, cols: u16, rows: u16) -> Result<AttachChannel> {
    let stream = connect(endpoint)
        .await
        .map_err(|_| anyhow::anyhow!("Daemon is not running."))?;
    let (reader, mut writer) = tokio::io::split(stream);
    let req = Request::SessionAttach {
        id: id.to_string(),
        cols,
        rows,
    };
    let mut line = serde_json::to_string(&req)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    Ok(AttachChannel {
        reader: BufReader::new(reader).lines(),
        writer,
    })
}

pub async fn approve_plan_node(plan_id: &str, node_id: &str) -> Result<()> {
    match send(&Request::ApprovePlanNode {
        plan_id: plan_id.to_string(),
        node_id: node_id.to_string(),
    })
    .await?
    {
        Response::PlanApproved { .. } => Ok(()),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn list_schedules() -> Result<Vec<orbit_core::schedule::ScheduledPlan>> {
    match send(&Request::ListSchedules).await? {
        Response::Schedules { schedules } => Ok(schedules),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn cancel_schedule(id: &str) -> Result<()> {
    match send(&Request::CancelSchedule { id: id.to_string() }).await? {
        Response::ScheduleCancelled { .. } => Ok(()),
        Response::Error { message } => bail!("{message}"),
        _ => bail!("unexpected response"),
    }
}

pub async fn launch_session(
    workspace: Option<String>,
    tenant: Option<String>,
    project: Option<String>,
    repository: Option<String>,
    engine: &str,
    no_tmux: bool,
    new_session: bool,
) -> Result<LaunchedInfo> {
    match send(&Request::LaunchSession {
        workspace,
        tenant,
        project,
        repository,
        engine: engine.to_string(),
        no_tmux,
        new_session,
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
