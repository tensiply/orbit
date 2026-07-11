use crate::audit::AuditStats;
use crate::eval::{EvalConstraint, EvalResult};
use crate::plan::Plan;
use crate::schedule::ScheduledPlan;
use crate::session::Session;
use serde::{Deserialize, Serialize};

// ── socket path ───────────────────────────────────────────────────────────────

/// `~/.local/share/orbit/orbit.sock`
pub fn socket_path() -> std::path::PathBuf {
    xdg_data_dir().join("orbit/orbit.sock")
}

/// `~/.local/share/orbit/orbitd.pid`
pub fn pid_path() -> std::path::PathBuf {
    xdg_data_dir().join("orbit/orbitd.pid")
}

fn xdg_data_dir() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        std::path::PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    }
}

// ── PlanStreamEvent ───────────────────────────────────────────────────────────

/// Events pushed by the daemon while a plan is executing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlanStreamEvent {
    NodeStarted { plan_id: String, node_id: String, label: String },
    NodeCompleted { plan_id: String, node_id: String },
    NodeFailed { plan_id: String, node_id: String, error: String },
    /// A single line of live output from a running node's tmux pane.
    NodeOutput { plan_id: String, node_id: String, line: String },
    PlanCompleted { plan_id: String },
    PlanFailed { plan_id: String },
    PlanReplanning { plan_id: String, child_plan_id: String },
}

impl PlanStreamEvent {
    pub fn plan_id(&self) -> &str {
        match self {
            Self::NodeStarted { plan_id, .. }
            | Self::NodeCompleted { plan_id, .. }
            | Self::NodeFailed { plan_id, .. }
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
    GetPlan {
        id: String,
    },
    ListPlans,
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
    CancelSchedule { id: String },
    /// Fire a scheduled plan immediately (ignoring next_run).
    RunScheduleNow { id: String },
    /// Request a rich diagnostics snapshot from the daemon.
    Health,
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
}
