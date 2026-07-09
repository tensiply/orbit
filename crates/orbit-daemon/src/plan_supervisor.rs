use orbit_core::{
    audit::{append_event, AuditEvent},
    memory::{append_plan_run, PlanRunRecord},
    plan::{NodeStatus, Plan, PlanScope, PlanStatus},
    session::Session,
};
use orbit_engine::{
    config, launcher,
    resolver::{self, ResolveArgs},
};
use orbit_planner::selector;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, warn};

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
            if let Err(e) = advance_plan(&mut plan) {
                warn!("advance_plan error for {}: {e}", plan.id);
            }
        }
    }
    Ok(())
}

fn advance_plan(plan: &mut Plan) -> anyhow::Result<()> {
    let all_sessions = Session::load_all();

    // ── 1. Check Running nodes for session completion ─────────────────────────
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
            info!("node {} completed in plan {}", node.id, plan.id);
        }
    }

    // ── 2. Dispatch ready Pending nodes ───────────────────────────────────────
    // Collect ids first to avoid simultaneous mutable + immutable borrow of plan.nodes
    let ready_ids: Vec<String> = plan.ready_nodes().iter().map(|n| n.id.clone()).collect();

    for node_id in ready_ids {
        let Some(idx) = plan.nodes.iter().position(|n| n.id == node_id) else {
            continue;
        };

        // Clone what dispatch_node needs — releases the immutable borrow
        let node_engine = plan.nodes[idx].engine;
        let node_intent = plan.nodes[idx].intent.clone();
        let node_label = plan.nodes[idx].label.clone();
        let dispatch_scope = plan.nodes[idx]
            .scope_override
            .clone()
            .unwrap_or_else(|| plan.scope.clone());

        // selector may override engine (e.g. Test → Opencode)
        let engine = selector::select(&plan.nodes[idx]).engine;
        let _ = node_engine; // advisory engine noted, selector wins

        match dispatch_node(&node_id, &node_label, &node_intent, &dispatch_scope, engine) {
            Ok(session) => {
                let node = &mut plan.nodes[idx];
                node.session_id = Some(session.id.clone());
                node.status = NodeStatus::Running;
                node.started_at = Some(now_secs());
                info!("dispatched node {} → session {} in plan {}", node_id, session.id, plan.id);
                let _ = append_event(&AuditEvent::NodeStarted {
                    plan_id: plan.id.clone(),
                    node_id: node_id.clone(),
                    engine: format!("{engine:?}"),
                    timestamp: now_secs(),
                });
            }
            Err(e) => {
                warn!("dispatch failed for node {node_id} in plan {}: {e}", plan.id);
                let node = &mut plan.nodes[idx];
                node.status = NodeStatus::Failed;
                node.error = Some(e.to_string());
                let _ = append_event(&AuditEvent::NodeFailed {
                    plan_id: plan.id.clone(),
                    node_id: node_id.clone(),
                    reason: e.to_string(),
                    timestamp: now_secs(),
                });
            }
        }
    }

    // ── 3. Check overall plan completion ─────────────────────────────────────
    let all_done = plan.nodes.iter().all(|n| {
        matches!(
            n.status,
            NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped
        )
    });

    if all_done && !plan.nodes.is_empty() {
        let any_failed = plan.nodes.iter().any(|n| n.status == NodeStatus::Failed);
        let outcome = if any_failed { PlanStatus::Failed } else { PlanStatus::Completed };
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

        info!("plan {} finished: {outcome:?}", plan.id);
    }

    plan.save()?;
    Ok(())
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

fn dispatch_node(
    node_id: &str,
    node_label: &str,
    node_intent: &str,
    scope: &PlanScope,
    engine: orbit_core::engine::Engine,
) -> anyhow::Result<Session> {
    let orbit_scope = resolver::resolve(ResolveArgs {
        workspace: scope.workspace.clone(),
        tenant: scope.tenant.clone(),
        project: scope.project.clone(),
        repository: scope.repository.clone(),
    })?;

    let mut merged = config::load(&orbit_scope, engine)?;

    // Inject node intent as a temporary instruction file
    let intent_path = write_node_intent(node_id, node_label, node_intent)?;
    merged.instructions.push(intent_path);

    launcher::spawn_background(&orbit_scope, &merged, engine, None)
}

fn write_node_intent(node_id: &str, label: &str, intent: &str) -> anyhow::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("orbit-plan-nodes");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{node_id}.md"));
    let content = format!("# Task: {label}\n\n{intent}\n\nComplete this task autonomously. When done, exit cleanly.\n");
    std::fs::write(&path, &content)?;
    Ok(path)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
