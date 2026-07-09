use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn xdg_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

fn audit_path() -> PathBuf {
    xdg_data_dir().join("orbit/audit.jsonl")
}

// ── AuditEvent ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum AuditEvent {
    PlanCreated {
        plan_id: String,
        intent: String,
        node_count: usize,
        timestamp: u64,
    },
    NodeStarted {
        plan_id: String,
        node_id: String,
        engine: String,
        timestamp: u64,
    },
    NodeCompleted {
        plan_id: String,
        node_id: String,
        duration_secs: u64,
        timestamp: u64,
    },
    NodeFailed {
        plan_id: String,
        node_id: String,
        reason: String,
        timestamp: u64,
    },
    ReplanTriggered {
        plan_id: String,
        from_node: String,
        replan_count: u8,
        timestamp: u64,
    },
    PlanCompleted {
        plan_id: String,
        outcome: String,
        total_duration_secs: u64,
        timestamp: u64,
    },
    PolicyBlocked {
        plan_id: String,
        node_id: String,
        reason: String,
        timestamp: u64,
    },
}

impl AuditEvent {
    pub fn plan_id(&self) -> &str {
        match self {
            AuditEvent::PlanCreated { plan_id, .. } => plan_id,
            AuditEvent::NodeStarted { plan_id, .. } => plan_id,
            AuditEvent::NodeCompleted { plan_id, .. } => plan_id,
            AuditEvent::NodeFailed { plan_id, .. } => plan_id,
            AuditEvent::ReplanTriggered { plan_id, .. } => plan_id,
            AuditEvent::PlanCompleted { plan_id, .. } => plan_id,
            AuditEvent::PolicyBlocked { plan_id, .. } => plan_id,
        }
    }

    pub fn with_timestamp(self) -> Self {
        let ts = now_secs();
        match self {
            AuditEvent::PlanCreated { plan_id, intent, node_count, .. } => {
                AuditEvent::PlanCreated { plan_id, intent, node_count, timestamp: ts }
            }
            AuditEvent::NodeStarted { plan_id, node_id, engine, .. } => {
                AuditEvent::NodeStarted { plan_id, node_id, engine, timestamp: ts }
            }
            AuditEvent::NodeCompleted { plan_id, node_id, duration_secs, .. } => {
                AuditEvent::NodeCompleted { plan_id, node_id, duration_secs, timestamp: ts }
            }
            AuditEvent::NodeFailed { plan_id, node_id, reason, .. } => {
                AuditEvent::NodeFailed { plan_id, node_id, reason, timestamp: ts }
            }
            AuditEvent::ReplanTriggered { plan_id, from_node, replan_count, .. } => {
                AuditEvent::ReplanTriggered { plan_id, from_node, replan_count, timestamp: ts }
            }
            AuditEvent::PlanCompleted { plan_id, outcome, total_duration_secs, .. } => {
                AuditEvent::PlanCompleted { plan_id, outcome, total_duration_secs, timestamp: ts }
            }
            AuditEvent::PolicyBlocked { plan_id, node_id, reason, .. } => {
                AuditEvent::PolicyBlocked { plan_id, node_id, reason, timestamp: ts }
            }
        }
    }
}

// ── API ───────────────────────────────────────────────────────────────────────

pub fn append_event(event: &AuditEvent) -> Result<()> {
    let path = audit_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(event)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn events_for_plan(plan_id: &str) -> Vec<AuditEvent> {
    let path = audit_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<AuditEvent>(line).ok())
        .filter(|e| e.plan_id() == plan_id)
        .collect()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn append_and_filter() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data").to_str().unwrap()); }

        let e = AuditEvent::PlanCreated {
            plan_id: "plan_abc123".into(),
            intent: "do stuff".into(),
            node_count: 2,
            timestamp: 0,
        };
        append_event(&e).unwrap();

        let e2 = AuditEvent::NodeStarted {
            plan_id: "plan_other".into(),
            node_id: "n0".into(),
            engine: "claude".into(),
            timestamp: 0,
        };
        append_event(&e2).unwrap();

        let events = events_for_plan("plan_abc123");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].plan_id(), "plan_abc123");
    }
}
