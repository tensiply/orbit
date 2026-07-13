use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::plan::Plan;

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
            AuditEvent::PlanCreated {
                plan_id,
                intent,
                node_count,
                ..
            } => AuditEvent::PlanCreated {
                plan_id,
                intent,
                node_count,
                timestamp: ts,
            },
            AuditEvent::NodeStarted {
                plan_id,
                node_id,
                engine,
                ..
            } => AuditEvent::NodeStarted {
                plan_id,
                node_id,
                engine,
                timestamp: ts,
            },
            AuditEvent::NodeCompleted {
                plan_id,
                node_id,
                duration_secs,
                ..
            } => AuditEvent::NodeCompleted {
                plan_id,
                node_id,
                duration_secs,
                timestamp: ts,
            },
            AuditEvent::NodeFailed {
                plan_id,
                node_id,
                reason,
                ..
            } => AuditEvent::NodeFailed {
                plan_id,
                node_id,
                reason,
                timestamp: ts,
            },
            AuditEvent::ReplanTriggered {
                plan_id,
                from_node,
                replan_count,
                ..
            } => AuditEvent::ReplanTriggered {
                plan_id,
                from_node,
                replan_count,
                timestamp: ts,
            },
            AuditEvent::PlanCompleted {
                plan_id,
                outcome,
                total_duration_secs,
                ..
            } => AuditEvent::PlanCompleted {
                plan_id,
                outcome,
                total_duration_secs,
                timestamp: ts,
            },
            AuditEvent::PolicyBlocked {
                plan_id,
                node_id,
                reason,
                ..
            } => AuditEvent::PolicyBlocked {
                plan_id,
                node_id,
                reason,
                timestamp: ts,
            },
        }
    }
}

// ── AuditStats ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditStats {
    pub total_plans: usize,
    pub completed_plans: usize,
    pub failed_plans: usize,
    pub avg_duration_secs: u64,
    pub total_nodes_dispatched: usize,
    pub total_nodes_completed: usize,
    pub total_nodes_failed: usize,
    /// Total estimated USD cost summed from all plan files (best-effort, 0.0 if no token data).
    pub total_cost_usd: f64,
    /// Total tokens consumed across all plans (prompt + completion).
    pub total_tokens: u64,
}

// ── API ───────────────────────────────────────────────────────────────────────

pub fn audit_stats() -> AuditStats {
    // Aggregate events from all workspace audit logs.
    let events: Vec<AuditEvent> = crate::data_paths::all_audit_paths()
        .into_iter()
        .filter_map(|p| fs::read_to_string(&p).ok())
        .flat_map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<AuditEvent>(line).ok())
                .collect::<Vec<_>>()
        })
        .collect();

    let mut stats = AuditStats::default();
    let mut total_duration: u64 = 0;
    let mut duration_count: u64 = 0;

    for event in &events {
        match event {
            AuditEvent::PlanCreated { .. } => stats.total_plans += 1,
            AuditEvent::PlanCompleted {
                outcome,
                total_duration_secs,
                ..
            } => {
                if outcome == "Completed" {
                    stats.completed_plans += 1;
                } else {
                    stats.failed_plans += 1;
                }
                total_duration += total_duration_secs;
                duration_count += 1;
            }
            AuditEvent::NodeStarted { .. } => stats.total_nodes_dispatched += 1,
            AuditEvent::NodeCompleted { .. } => stats.total_nodes_completed += 1,
            AuditEvent::NodeFailed { .. } => stats.total_nodes_failed += 1,
            _ => {}
        }
    }

    stats.avg_duration_secs = total_duration.checked_div(duration_count).unwrap_or(0);

    // Aggregate cost and token data from live plan files (both active and archived).
    for plan in Plan::load_all() {
        for node in &plan.nodes {
            if let Some(ref usage) = node.token_usage {
                stats.total_cost_usd += usage.estimated_cost_usd;
                stats.total_tokens += usage.prompt_tokens + usage.completion_tokens;
            }
        }
    }

    stats
}

pub fn append_event(event: &AuditEvent) -> Result<()> {
    append_event_for(None, event)
}

/// Write an audit event to a specific workspace's audit log.
/// Pass `workspace_name = None` to write to the legacy flat path.
pub fn append_event_for(workspace_name: Option<&str>, event: &AuditEvent) -> Result<()> {
    let path = crate::data_paths::audit_path_for(workspace_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(event)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn events_for_plan(plan_id: &str) -> Vec<AuditEvent> {
    // Search all workspace audit logs.
    crate::data_paths::all_audit_paths()
        .into_iter()
        .filter_map(|p| fs::read_to_string(&p).ok())
        .flat_map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<AuditEvent>(line).ok())
                .filter(|e| e.plan_id() == plan_id)
                .collect::<Vec<_>>()
        })
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
        unsafe {
            std::env::set_var("XDG_DATA_HOME", tmp.path().join("data").to_str().unwrap());
        }

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
