//! Daemon-owned PTY backend.
//!
//! On unix, AI sessions live in tmux (a separate server that survives daemon
//! restarts). Windows has no tmux, so the daemon owns each session's PTY
//! directly via `portable-pty`/ConPTY, keeps a bounded scrollback ring, and
//! fans live output out to attached clients over IPC (`AttachFrame`). The PTY
//! survives client disconnects but dies with the daemon (accepted v1 limit).
//!
//! The backend is cross-platform: it is the default on Windows and available on
//! unix opt-in via `ORBIT_DAEMON_PTY=1`, which lets the whole attach/reattach
//! flow be exercised on Linux/CI. See ADR-012 / ADR-013.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use orbit_core::context::OrbitScope;
use orbit_core::engine::Engine;
use orbit_core::jira::TaskContext;
use orbit_core::session::{Session, SessionBackendKind};
use orbit_engine::config::MergedConfig;
use orbit_engine::launcher::backend::SessionBackend;
use orbit_engine::launcher::{LaunchKind, PreparedLaunch, prepare_launch};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tokio::sync::broadcast;

/// Scrollback retained per session for reattach (bytes).
const SCROLLBACK_CAP: usize = 256 * 1024;
/// Live-output broadcast buffer (chunks) — lagging attachers drop chunks, not bytes
/// from the ring (a fresh attach always replays the ring first).
const BROADCAST_CAP: usize = 1024;

/// A live daemon-owned PTY. Shared (`Arc`) between the reader thread, the attach
/// handler, and one-shot resize/detach requests.
pub struct PtyHandle {
    master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    /// Bounded scrollback ring. Also the serialization point between the reader
    /// thread (append + broadcast) and `attach` (snapshot + subscribe), so a
    /// reattaching client sees every byte exactly once.
    scrollback: Arc<Mutex<VecDeque<u8>>>,
    output_tx: broadcast::Sender<Vec<u8>>,
}

impl PtyHandle {
    /// Snapshot the scrollback backlog and subscribe to live output atomically:
    /// holding the scrollback lock across both means the reader thread cannot
    /// interleave an append+broadcast between them, so bytes are neither dropped
    /// nor duplicated at the handoff.
    pub fn attach(&self) -> (Vec<u8>, broadcast::Receiver<Vec<u8>>) {
        let sb = self.scrollback.lock().unwrap();
        let backlog: Vec<u8> = sb.iter().copied().collect();
        let rx = self.output_tx.subscribe();
        drop(sb);
        (backlog, rx)
    }

    /// Write client input (keystrokes / stdin) into the PTY.
    pub fn write_input(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(bytes)?;
        w.flush()
    }

    /// Resize the PTY to the client's terminal dimensions.
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.master.lock().unwrap().resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}

type Registry = Arc<Mutex<HashMap<String, Arc<PtyHandle>>>>;

/// Process-global registry of live PTYs, keyed by `Session::id`. Reached by the
/// backend (spawn), the attach streaming handler, and resize/detach requests
/// without threading state through the daemon's call graph.
static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

/// Look up a live PTY by session id.
pub fn get(session_id: &str) -> Option<Arc<PtyHandle>> {
    registry().lock().unwrap().get(session_id).cloned()
}

// ── backend ─────────────────────────────────────────────────────────────────

/// Session backend that runs the engine inside a daemon-owned PTY.
pub struct DaemonPtyBackend;

impl SessionBackend for DaemonPtyBackend {
    fn spawn_background(
        &self,
        scope: &OrbitScope,
        config: &MergedConfig,
        engine: Engine,
        task_context: Option<&TaskContext>,
        _session_name: Option<&str>,
        _force_new: bool,
    ) -> Result<Session> {
        let prepared = prepare_launch(
            scope,
            config,
            engine,
            LaunchKind::Interactive { task_context },
        )?;
        spawn_prepared(scope, engine, prepared)
    }

    fn spawn_plan_node(
        &self,
        _session_name: &str,
        intent: &str,
        scope: &OrbitScope,
        config: &MergedConfig,
        engine: Engine,
    ) -> Result<Session> {
        let prepared = prepare_launch(scope, config, engine, LaunchKind::PlanNode { intent })?;
        spawn_prepared(scope, engine, prepared)
    }
}

/// Build the `portable-pty` command for a prepared launch. On Windows the engine
/// is often an npm shim (`claude.cmd`) that ConPTY cannot execute directly, so
/// route it through `cmd /c`; on unix run the binary directly.
#[cfg(windows)]
fn pty_command(prepared: &PreparedLaunch) -> CommandBuilder {
    let mut cmd = CommandBuilder::new("cmd");
    cmd.arg("/c");
    cmd.arg(&prepared.bin);
    for arg in &prepared.args {
        cmd.arg(arg);
    }
    cmd
}

#[cfg(unix)]
fn pty_command(prepared: &PreparedLaunch) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(&prepared.bin);
    for arg in &prepared.args {
        cmd.arg(arg);
    }
    cmd
}

fn spawn_prepared(scope: &OrbitScope, engine: Engine, prepared: PreparedLaunch) -> Result<Session> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut builder = pty_command(&prepared);
    for (k, v) in &prepared.env {
        builder.env(k, v);
    }
    builder.cwd(&prepared.work_dir);

    let mut child = pair.slave.spawn_command(builder)?;
    let pid = child.process_id().unwrap_or(0);
    // Drop the slave in the daemon so the reader sees EOF when the child exits.
    drop(pair.slave);

    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(BROADCAST_CAP);
    let scrollback = Arc::new(Mutex::new(VecDeque::with_capacity(SCROLLBACK_CAP)));

    let handle = Arc::new(PtyHandle {
        master: Mutex::new(pair.master),
        writer: Mutex::new(writer),
        scrollback: scrollback.clone(),
        output_tx: output_tx.clone(),
    });

    let mut session = Session::new(
        pid,
        engine.as_str(),
        &scope.tenant,
        &scope.project,
        &scope.repository,
        scope.work_dir.clone(),
        scope.global_mode,
        None,
    );
    session.backend = SessionBackendKind::DaemonPty;
    let id = session.id.clone();
    registry().lock().unwrap().insert(id.clone(), handle);

    // Reader thread: drain the PTY → append to the ring + broadcast, holding the
    // scrollback lock across both so `attach` gets a clean handoff. On EOF, reap
    // the child and drop the session from the registry.
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = buf[..n].to_vec();
                    let mut sb = scrollback.lock().unwrap();
                    for &b in &chunk {
                        if sb.len() >= SCROLLBACK_CAP {
                            sb.pop_front();
                        }
                        sb.push_back(b);
                    }
                    let _ = output_tx.send(chunk);
                    drop(sb);
                }
            }
        }
        let _ = child.wait();
        registry().lock().unwrap().remove(&id);
    });

    if let Err(e) = session.save() {
        tracing::warn!("could not save daemon-pty session: {e}");
    }
    Ok(session)
}

/// Select the session backend for this daemon: the daemon-owned PTY on Windows
/// or when `ORBIT_DAEMON_PTY` is set (unix opt-in), otherwise tmux.
pub fn select_backend() -> Box<dyn SessionBackend> {
    let use_pty = cfg!(windows) || std::env::var_os("ORBIT_DAEMON_PTY").is_some();
    if use_pty {
        Box::new(DaemonPtyBackend)
    } else {
        orbit_engine::launcher::backend::session_backend()
    }
}
