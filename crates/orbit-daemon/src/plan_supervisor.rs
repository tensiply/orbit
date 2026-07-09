use orbit_core::{
    audit::{append_event, AuditEvent},
    memory::{append_plan_run, PlanRunRecord},
    plan::{NodeStatus, Plan, PlanStatus},
};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::warn;

pub async fn run_supervisor_loop(
    interval: Duration,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(interval) => {
                if let Err(e) = tick() {
                    warn!("supervisor tick error: {e}");
                }
            }
        }
    }
}

fn tick() -> anyhow::Result<()> {
    let plans = Plan::load_all();
    for mut plan in plans {
        if plan.status == PlanStatus::Running {
            advance_plan(&mut plan)?;
        }
    }
    Ok(())
}

fn advance_plan(plan: &mut Plan) -> anyhow::Result<()> {
    let all_sessions = orbit_core::session::Session::load_all();

    for node in plan.nodes.iter_mut() {
        if node.status != NodeStatus::Running {
            continue;
        }
        let Some(ref session_id) = node.session_id else {
            continue;
        };
        let session_opt = all_sessions.iter().find(|s| &s.id == session_id);
        let is_done = match session_opt {
            None => true,
            Some(s) => !s.is_running(),
        };
        if is_done {
            node.status = NodeStatus::Completed;
            node.completed_at = Some(now_secs());
            let _ = append_event(&AuditEvent::NodeCompleted {
                plan_id: plan.id.clone(),
                node_id: node.id.clone(),
                duration_secs: node.started_at.map(|s| now_secs().saturating_sub(s)).unwrap_or(0),
                timestamp: now_secs(),
            });
        }
    }

    let all_done = plan.nodes.iter().all(|n| {
        matches!(
            n.status,
            NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped
        )
    });

    if all_done {
        let any_failed = plan.nodes.iter().any(|n| n.status == NodeStatus::Failed);
        let outcome = if any_failed {
            PlanStatus::Failed
        } else {
            PlanStatus::Completed
        };
        let duration = now_secs().saturating_sub(plan.created_at);
        plan.status = outcome.clone();
        plan.completed_at = Some(now_secs());

        let _ = append_event(&AuditEvent::PlanCompleted {
            plan_id: plan.id.clone(),
            outcome: format!("{outcome:?}"),
            total_duration_secs: duration,
            timestamp: now_secs(),
        });

        let _ = append_plan_run(&PlanRunRecord {
            plan_id: plan.id.clone(),
            intent: plan.intent.clone(),
            outcome: format!("{outcome:?}"),
            node_count: plan.nodes.len(),
            replan_count: plan.replan_count,
            duration_secs: duration,
            created_at: plan.created_at,
            scope_key: plan.scope.scope_key(),
            tags: vec![],
        });
    }

    plan.save()?;
    Ok(())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
