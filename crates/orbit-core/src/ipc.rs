use crate::audit::AuditStats;
use crate::eval::{EvalConstraint, EvalResult};
use crate::plan::Plan;
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

// ── PlannerTrace ──────────────────────────────────────────────────────────────

/// Verbose debug data captured during planner invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerTrace {
    pub system_prompt: String,
    pub user_prompt: String,
    pub raw_response: String,
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
}
