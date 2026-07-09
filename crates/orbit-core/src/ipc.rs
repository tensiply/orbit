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
    },
    GetPlan {
        id: String,
    },
    ListPlans,
    CancelPlan {
        id: String,
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
}
