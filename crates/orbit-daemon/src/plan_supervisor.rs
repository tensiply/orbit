use orbit_core::{
    audit::{append_event, AuditEvent},
    engine::Engine,
    memory::{append_plan_run, load_recent_runs, PlanRunRecord},
    plan::{NodeStatus, Plan, PlanScope, PlanStatus},
    session::Session,
};
use orbit_engine::{
    config, launcher,
    resolver::{self, ResolveArgs},
};
use orbit_planner::{
    planner::PlannerConfig,
    replanner,
    selector,
    verifier::{verify_node, VerifyOutcome},
};
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
        let Some(ref session_id) = node.session_id.clone() else {
            continue;
        };
        let session_opt = all_sessions.iter().find(|s| &s.id == session_id);
        let is_done = match session_opt {
            None => true,
            Some(s) => !s.is_running(),
        };
        if !is_done {
            continue;
        }

        // Capture pane output before classifying the node outcome
        node.output_summary = capture_node_output(&node.id);

        let outcome = verify_node(node, node.engine);
        match outcome {
            VerifyOutcome::Pass => {
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

            VerifyOutcome::Fail(reason) => {
                if node.retry_count < node.policy.retry_max {
                    // Retry: reset node to Pending so it gets dispatched again
                    node.retry_count += 1;
                    node.status = NodeStatus::Pending;
                    node.session_id = None;
                    node.started_at = None;
                    node.output_summary = None;
                    info!(
                        "retrying node {} ({}/{}) in plan {}",
                        node.id, node.retry_count, node.policy.retry_max, plan.id
                    );
                } else {
                    node.status = NodeStatus::Failed;
                    node.completed_at = Some(now_secs());
                    node.error = Some(reason.clone());
                    let _ = append_event(&AuditEvent::NodeFailed {
                        plan_id: plan.id.clone(),
                        node_id: node.id.clone(),
                        reason: reason.clone(),
                        timestamp: now_secs(),
                    });
                    warn!("node {} failed in plan {}: {reason}", node.id, plan.id);
                }
            }
        }
    }

    // ── 2. Dispatch ready Pending nodes ───────────────────────────────────────
    let ready_ids: Vec<String> = plan.ready_nodes().iter().map(|n| n.id.clone()).collect();

    for node_id in ready_ids {
        let Some(idx) = plan.nodes.iter().position(|n| n.id == node_id) else {
            continue;
        };

        let node_engine = plan.nodes[idx].engine;
        let node_intent = plan.nodes[idx].intent.clone();
        let node_label = plan.nodes[idx].label.clone();
        let dispatch_scope = plan.nodes[idx]
            .scope_override
            .clone()
            .unwrap_or_else(|| plan.scope.clone());

        let engine = selector::select(&plan.nodes[idx]).engine;
        let _ = node_engine;

        match dispatch_node(&plan.id, &node_id, &node_label, &node_intent, &dispatch_scope, engine) {
            Ok(session) => {
                start_output_capture(&node_id, &session);
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

        // Replan if there are failures and budget allows
        if any_failed && plan.replan_count < plan.policy.max_replan_count {
            match try_replan(plan) {
                Ok(child) => {
                    let child_id = child.id.clone();
                    if let Err(e) = child.save() {
                        warn!("failed to save child plan {child_id}: {e}");
                    } else {
                        plan.status = PlanStatus::Replanning;
                        plan.save()?;
                        info!("plan {} replanning → child plan {child_id}", plan.id);
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!("replanning failed for plan {}: {e} — marking Failed", plan.id);
                }
            }
        }

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

// ── Replanning ────────────────────────────────────────────────────────────────

fn try_replan(plan: &Plan) -> anyhow::Result<Plan> {
    let failed_node = plan
        .nodes
        .iter()
        .find(|n| n.status == NodeStatus::Failed)
        .ok_or_else(|| anyhow::anyhow!("no failed node for replanning"))?;

    let reason = failed_node.error.as_deref().unwrap_or("verification failed");
    let recent_runs = load_recent_runs(5);
    let replan_engine = plan.planner_model.parse::<Engine>().unwrap_or(Engine::Claude);

    let cfg = PlannerConfig {
        engine: replan_engine,
        system_prompt_path: None,
    };

    replanner::replan(plan, failed_node, reason, &recent_runs, &cfg)
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

fn dispatch_node(
    plan_id: &str,
    node_id: &str,
    node_label: &str,
    node_intent: &str,
    scope: &PlanScope,
    engine: Engine,
) -> anyhow::Result<Session> {
    let orbit_scope = resolver::resolve(ResolveArgs {
        workspace: scope.workspace.clone(),
        tenant: scope.tenant.clone(),
        project: scope.project.clone(),
        repository: scope.repository.clone(),
    })?;

    let mut merged = config::load(&orbit_scope, engine)?;

    let intent_path = write_node_intent(node_id, node_label, node_intent)?;
    merged.instructions.push(intent_path);

    // Unique per-node session so plan nodes don't share the user's interactive session
    let plan_suffix = plan_id.trim_start_matches("plan_");
    let session_name = format!("orbit-plan-{plan_suffix}-{node_id}");

    launcher::spawn_plan_node(&session_name, node_intent, &orbit_scope, &merged, engine)
}

// ── Output capture ────────────────────────────────────────────────────────────

/// Start piping tmux pane output to a per-node log file.
fn start_output_capture(node_id: &str, session: &Session) {
    let Some(ref tmux_name) = session.tmux_session else {
        return;
    };
    let log_dir = std::env::temp_dir().join("orbit-plan-nodes");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("{node_id}.log"));
    // pipe-pane streams pane output to the file as the session runs
    let _ = std::process::Command::new("tmux")
        .args([
            "pipe-pane",
            "-t",
            tmux_name,
            &format!("cat >> {}", log_path.to_string_lossy()),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Read the last 100 lines from the node's log file.
fn capture_node_output(node_id: &str) -> Option<String> {
    let log_path = std::env::temp_dir()
        .join("orbit-plan-nodes")
        .join(format!("{node_id}.log"));
    let content = std::fs::read_to_string(&log_path).ok()?;
    if content.is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(100);
    Some(lines[start..].join("\n"))
}

fn write_node_intent(node_id: &str, label: &str, intent: &str) -> anyhow::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("orbit-plan-nodes");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{node_id}.md"));
    let content = format!(
        "# Task: {label}\n\n{intent}\n\nComplete this task autonomously. When done, exit cleanly.\n"
    );
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
