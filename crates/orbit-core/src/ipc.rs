use crate::audit::AuditStats;
use crate::eval::{EvalConstraint, EvalResult};
use crate::plan::Plan;
use crate::schedule::ScheduledPlan;
use crate::session::Session;
use serde::{Deserialize, Serialize};

// ── socket path ───────────────────────────────────────────────────────────────

/// `~/.orbit/run/orbit.sock`
pub fn socket_path() -> std::path::PathBuf {
    crate::data_paths::orbit_run_dir().join("orbit.sock")
}

/// `~/.orbit/run/orbitd.pid`
pub fn pid_path() -> std::path::PathBuf {
    crate::data_paths::orbit_run_dir().join("orbitd.pid")
}

// ── endpoint names ──────────────────────────────────────────────────────────────
//
// The daemon IPC runs over a local socket. On unix that is a Unix domain socket
// living at a filesystem path; on Windows it is a named pipe under `\\.\pipe\`.
// These helpers hand the transport layer (orbit-client / orbit-daemon) a
// platform-appropriate endpoint name — the wire protocol on top is identical.

/// Endpoint name for the main daemon socket.
#[cfg(unix)]
pub fn socket_endpoint() -> String {
    socket_path().to_string_lossy().into_owned()
}

/// Endpoint name for the main daemon socket.
#[cfg(windows)]
pub fn socket_endpoint() -> String {
    // Channel-scoped so stable/canary/dev daemons never collide on one pipe.
    format!(
        r"\\.\pipe\orbit{}",
        crate::channel::Channel::current().home_suffix()
    )
}

/// Map a project-socket filesystem path to a platform endpoint name. Project
/// sockets are addressed by path across the codebase; on Windows a named pipe
/// cannot live at an arbitrary path, so derive a stable pipe name from it.
#[cfg(unix)]
pub fn endpoint_for_path(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Map a project-socket filesystem path to a platform endpoint name.
#[cfg(windows)]
pub fn endpoint_for_path(path: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    format!(r"\\.\pipe\orbit-proj-{:016x}", h.finish())
}

// ── PlanStreamEvent ───────────────────────────────────────────────────────────

/// Events pushed by the daemon while a plan is executing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlanStreamEvent {
    NodeStarted {
        plan_id: String,
        node_id: String,
        label: String,
    },
    NodeCompleted {
        plan_id: String,
        node_id: String,
    },
    NodeFailed {
        plan_id: String,
        node_id: String,
        error: String,
    },
    /// A node entered AwaitingApproval — the TUI can render an inline approve gate.
    NodeAwaitingApproval {
        plan_id: String,
        node_id: String,
        label: String,
    },
    /// A single line of live output from a running node's tmux pane.
    NodeOutput {
        plan_id: String,
        node_id: String,
        line: String,
    },
    PlanCompleted {
        plan_id: String,
    },
    PlanFailed {
        plan_id: String,
    },
    PlanReplanning {
        plan_id: String,
        child_plan_id: String,
    },
}

impl PlanStreamEvent {
    pub fn plan_id(&self) -> &str {
        match self {
            Self::NodeStarted { plan_id, .. }
            | Self::NodeCompleted { plan_id, .. }
            | Self::NodeFailed { plan_id, .. }
            | Self::NodeAwaitingApproval { plan_id, .. }
            | Self::NodeOutput { plan_id, .. }
            | Self::PlanCompleted { plan_id }
            | Self::PlanFailed { plan_id }
            | Self::PlanReplanning { plan_id, .. } => plan_id,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::PlanCompleted { .. } | Self::PlanFailed { .. })
    }
}

// ── PlannerTrace ──────────────────────────────────────────────────────────────

/// Verbose debug data captured during planner invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerTrace {
    pub system_prompt: String,
    pub user_prompt: String,
    pub raw_response: String,
}

// ── project socket role ───────────────────────────────────────────────────────

/// Role granted to connections on a project socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    /// Can read plan state and approve AwaitingApproval nodes.
    #[default]
    Contributor,
    /// Read-only; cannot approve nodes.
    Observer,
}

// ── protocol ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    ListSessions,
    KillSession {
        id: String,
    },
    CleanSessions,
    Status,
    Shutdown,
    LaunchSession {
        workspace: Option<String>,
        tenant: Option<String>,
        project: Option<String>,
        repository: Option<String>,
        engine: String,
        no_tmux: bool,
        #[serde(default)]
        new_session: bool,
    },
    CreatePlan {
        intent: String,
        workspace: Option<String>,
        tenant: Option<String>,
        project: Option<String>,
        repository: Option<String>,
        dry_run: bool,
        #[serde(default)]
        verbose: bool,
        #[serde(default)]
        extra_repos: Vec<crate::plan::CrossRepoSpec>,
        /// Override max token budget for this plan (None = use user config default).
        #[serde(default)]
        max_tokens: Option<u64>,
        /// Override max wall-clock duration in seconds (None = use user config default).
        #[serde(default)]
        max_duration_secs: Option<u64>,
        /// Override max estimated USD cost (None = use user config default).
        #[serde(default)]
        max_cost_usd: Option<f64>,
        /// Override max dispatched node count (None = use user config default).
        #[serde(default)]
        max_nodes: Option<u32>,
    },
    /// Attach to a session's daemon-owned PTY (streaming, bidirectional). After
    /// this request the connection carries `AttachFrame`s instead of `Response`s:
    /// the daemon streams backlog + live `Output`, the client sends `Input` /
    /// `Resize` / `Detach`. Only meaningful for `SessionBackendKind::DaemonPty`.
    SessionAttach {
        id: String,
        cols: u16,
        rows: u16,
    },
    /// Resize a daemon-owned PTY (standalone, outside an attach stream).
    SessionResize {
        id: String,
        cols: u16,
        rows: u16,
    },
    /// Detach without terminating the session (the daemon keeps the PTY alive).
    SessionDetach {
        id: String,
    },
    GetPlan {
        id: String,
    },
    ListPlans {
        /// Filter plans by workspace name. None = all workspaces.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace_filter: Option<String>,
    },
    CancelPlan {
        id: String,
    },
    ApprovePlanNode {
        plan_id: String,
        node_id: String,
    },
    GetPlanStats,
    EvalPlan {
        intent: String,
        workspace: Option<String>,
        tenant: Option<String>,
        project: Option<String>,
        repository: Option<String>,
        constraints: Vec<EvalConstraint>,
    },
    RetryPlan {
        id: String,
    },
    /// Freeze dispatch — Running nodes continue but no new nodes are started.
    PausePlan {
        id: String,
    },
    /// Resume a Paused plan.
    ResumePlan {
        id: String,
    },
    /// Subscribe to live events for a running plan (streaming response).
    StreamPlan {
        id: String,
    },
    /// Tell the daemon to start a restricted listener at the given path.
    AddProjectSocket {
        path: String,
        #[serde(default)]
        role: ProjectRole,
    },
    /// Create a new scheduled plan (once or cron).
    CreateSchedule {
        intent: String,
        /// Unix timestamp for a one-shot schedule.
        at: Option<u64>,
        /// 5-field cron expression for a recurring schedule.
        cron: Option<String>,
        #[serde(default)]
        repos: Vec<String>,
        workspace: Option<String>,
        tenant: Option<String>,
        project: Option<String>,
        repository: Option<String>,
    },
    /// List all scheduled plans.
    ListSchedules,
    /// Delete a scheduled plan.
    CancelSchedule {
        id: String,
    },
    /// Fire a scheduled plan immediately (ignoring next_run).
    RunScheduleNow {
        id: String,
    },
    /// Request a rich diagnostics snapshot from the daemon.
    Health,
    /// Start TCP serving with JWT-authenticated access.
    StartServing {
        port: u16,
        #[serde(default)]
        max_role: ProjectRole,
        #[serde(default)]
        name: String,
    },
    /// Stop TCP serving and mDNS announcement.
    StopServing,
    /// List network peers currently connected via TCP.
    ListNetworkPeers,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Sessions {
        sessions: Vec<Session>,
    },
    Killed {
        id: String,
    },
    Cleaned {
        count: usize,
    },
    Status {
        uptime_secs: u64,
        session_count: usize,
        pid: u32,
        /// Channel the daemon is serving (`stable`/`canary`/`dev`) — lets a
        /// caller confirm it reached the daemon for the channel it expected.
        #[serde(default)]
        channel: String,
        /// Resolved orbit home the daemon is bound to (e.g. `~/.orbit-canary`).
        #[serde(default)]
        home: String,
    },
    Launched {
        tmux_name: String,
        session_id: String,
    },
    Ok,
    Error {
        message: String,
    },
    PlanCreated {
        id: String,
        node_count: usize,
        /// Minimal node list for TUI inline widget — avoids a GetPlan roundtrip.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        nodes: Vec<crate::plan::NodeSummary>,
        /// Confidence level of scope detection: "High" | "Medium" | "Ambiguous" | "Fallback".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope_confidence: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        trace: Option<PlannerTrace>,
    },
    PlanInfo {
        plan: Plan,
    },
    Plans {
        plans: Vec<Plan>,
    },
    PlanCancelled {
        id: String,
    },
    PlanApproved {
        plan_id: String,
        node_id: String,
    },
    PlanStats {
        stats: AuditStats,
    },
    PlanEvalResult {
        plan: Plan,
        result: EvalResult,
    },
    PlanRetried {
        id: String,
        reset_count: usize,
    },
    PlanPaused {
        id: String,
    },
    PlanResumed {
        id: String,
    },
    ProjectSocketAdded {
        path: String,
    },
    ScheduleCreated {
        id: String,
        next_run: Option<u64>,
    },
    Schedules {
        schedules: Vec<ScheduledPlan>,
    },
    ScheduleCancelled {
        id: String,
    },
    ScheduleFired {
        schedule_id: String,
        plan_id: String,
    },
    Health {
        uptime_secs: u64,
        pid: u32,
        running_plans: usize,
        completed_today: usize,
        failed_today: usize,
        plan_files: usize,
        archived_plans: usize,
        memory_records: usize,
        auto_prune_enabled: bool,
        auto_prune_days: u32,
    },
    ServingStarted {
        port: u16,
        observer_token: String,
        contributor_token: Option<String>,
    },
    ServingStopped,
    NetworkPeers {
        peers: Vec<crate::net::NetworkPeerInfo>,
    },
}

// ── AttachFrame ─────────────────────────────────────────────────────────────────

/// Frames exchanged over a connection after a `SessionAttach`, replacing the
/// request/response protocol for the life of the attachment. One JSON object per
/// line (same newline framing as the rest of the protocol); raw PTY bytes ride
/// inside base64-encoded, which keeps the wire compact (a JSON `Vec<u8>` expands
/// each byte to a decimal number plus comma — ~4× overhead on binary streams).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum AttachFrame {
    /// daemon → client: PTY output (scrollback backlog first, then live).
    Output {
        #[serde(with = "base64_bytes")]
        bytes: Vec<u8>,
    },
    /// client → daemon: keystrokes / stdin to write into the PTY.
    Input {
        #[serde(with = "base64_bytes")]
        bytes: Vec<u8>,
    },
    /// client → daemon: terminal was resized.
    Resize { cols: u16, rows: u16 },
    /// client → daemon: detach and close the stream (PTY stays alive).
    Detach,
}

/// Serde helper: encode `Vec<u8>` as a base64 string on the wire instead of a
/// JSON array of byte integers.
mod base64_bytes {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(deserializer)?;
        STANDARD
            .decode(s.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_frame_output_roundtrips_as_base64() {
        let frame = AttachFrame::Output {
            bytes: vec![0x00, 0x1b, b'[', b'0', b'm', 0xff],
        };
        let json = serde_json::to_string(&frame).unwrap();
        // Bytes ride as a base64 string, not a JSON array of integers.
        assert!(json.contains("\"bytes\":\""));
        assert!(!json.contains('['));

        match serde_json::from_str::<AttachFrame>(&json).unwrap() {
            AttachFrame::Output { bytes } => {
                assert_eq!(bytes, vec![0x00, 0x1b, b'[', b'0', b'm', 0xff])
            }
            other => panic!("expected Output, got {other:?}"),
        }
    }
}
