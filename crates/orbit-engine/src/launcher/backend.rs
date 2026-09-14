//! Session backend abstraction.
//!
//! A *session backend* owns how an AI session is spawned detached and how it
//! later persists and reattaches. On unix this is `tmux` (a separate server that
//! survives client and daemon restarts). Windows has no tmux, so the daemon will
//! own the PTY directly — see [`orbit_core::session::SessionBackendKind`] and A2.
//!
//! This module introduces the seam only: the unix path keeps delegating to the
//! existing tmux free functions in [`super`], so behaviour is unchanged. The
//! Windows `DaemonPtyBackend` lands in a follow-up; until then Windows gets an
//! [`UnsupportedBackend`] that fails with a clear message instead of a cryptic
//! missing-tmux error.

use anyhow::Result;
use orbit_core::{context::OrbitScope, engine::Engine, jira::TaskContext, session::Session};

use crate::config::MergedConfig;

/// How the daemon spawns detached engine sessions. Kept minimal — only the
/// daemon-facing spawn operations, which are all that differ across platforms.
pub trait SessionBackend {
    /// Spawn a detached interactive engine session (daemon use). Mirrors the
    /// scope-derived naming and reuse semantics of the underlying implementation.
    fn spawn_background(
        &self,
        scope: &OrbitScope,
        config: &MergedConfig,
        engine: Engine,
        task_context: Option<&TaskContext>,
        session_name: Option<&str>,
        force_new: bool,
    ) -> Result<Session>;

    /// Spawn a headless plan-node engine session with an explicit intent.
    fn spawn_plan_node(
        &self,
        session_name: &str,
        intent: &str,
        scope: &OrbitScope,
        config: &MergedConfig,
        engine: Engine,
    ) -> Result<Session>;
}

/// tmux-backed sessions (unix). Delegates to the existing launcher free
/// functions verbatim — no behaviour change.
#[cfg(unix)]
pub struct TmuxBackend;

#[cfg(unix)]
impl SessionBackend for TmuxBackend {
    fn spawn_background(
        &self,
        scope: &OrbitScope,
        config: &MergedConfig,
        engine: Engine,
        task_context: Option<&TaskContext>,
        session_name: Option<&str>,
        force_new: bool,
    ) -> Result<Session> {
        super::spawn_background(scope, config, engine, task_context, session_name, force_new)
    }

    fn spawn_plan_node(
        &self,
        session_name: &str,
        intent: &str,
        scope: &OrbitScope,
        config: &MergedConfig,
        engine: Engine,
    ) -> Result<Session> {
        super::spawn_plan_node(session_name, intent, scope, config, engine)
    }
}

/// Placeholder backend for Windows until the daemon-owned PTY backend lands.
/// Fails every spawn with a clear message — no worse than today (tmux is absent
/// on Windows), but explicit about why.
#[cfg(windows)]
pub struct UnsupportedBackend;

#[cfg(windows)]
impl SessionBackend for UnsupportedBackend {
    fn spawn_background(
        &self,
        _scope: &OrbitScope,
        _config: &MergedConfig,
        _engine: Engine,
        _task_context: Option<&TaskContext>,
        _session_name: Option<&str>,
        _force_new: bool,
    ) -> Result<Session> {
        anyhow::bail!("daemon-PTY session backend not yet implemented on Windows")
    }

    fn spawn_plan_node(
        &self,
        _session_name: &str,
        _intent: &str,
        _scope: &OrbitScope,
        _config: &MergedConfig,
        _engine: Engine,
    ) -> Result<Session> {
        anyhow::bail!("daemon-PTY session backend not yet implemented on Windows")
    }
}

/// The session backend for the current platform.
#[cfg(unix)]
pub fn session_backend() -> Box<dyn SessionBackend> {
    Box::new(TmuxBackend)
}

/// The session backend for the current platform.
#[cfg(windows)]
pub fn session_backend() -> Box<dyn SessionBackend> {
    Box::new(UnsupportedBackend)
}
