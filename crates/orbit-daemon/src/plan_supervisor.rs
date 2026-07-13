use orbit_core::{
    audit::{append_event_for, AuditEvent},
    engine::Engine,
    hooks::{run_hooks, HookEvent},
    ipc::PlanStreamEvent,
    memory::{append_plan_run_for, load_recent_runs, PlanRunRecord},
    plan::{NodeStatus, Plan, PlanNode, PlanNodeType, PlanScope, PlanStatus, TokenUsage},
    session::Session,
};

/// Extract the workspace name from a plan for workspace-scoped storage routing.
#[inline]
fn ws(plan: &Plan) -> Option<&str> {
    plan.scope.workspace.as_deref()
}
use orbit_engine::{
    config, launcher,
    resolver::{self, ResolveArgs},
};
use orbit_planner::{
    backend::CliBackend,
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
    event_tx: broadcast::Sender<PlanStreamEvent>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(interval) => {
                if let Err(e) = tick(&event_tx) {
                    warn!("supervisor tick error: {e}");
                }
            }
        }
    }
}

fn tick(event_tx: &broadcast::Sender<PlanStreamEvent>) -> anyhow::Result<()> {
    let plans = Plan::load_all();
    for mut plan in plans
        .iter()
        .filter(|p| p.status == PlanStatus::Running || p.status == PlanStatus::Paused)
        .cloned()
    {
        if let Err(e) = advance_plan(&mut plan, event_tx) {
            warn!("advance_plan error for {}: {e}", plan.id);
        }
    }
    for mut plan in plans.iter().filter(|p| p.status == PlanStatus::Replanning).cloned() {
        if let Err(e) = close_replanning_plan(&mut plan, &plans, event_tx) {
            warn!("close_replanning error for {}: {e}", plan.id);
        }
    }
    Ok(())
}

/// Propagate a child plan's terminal outcome back to its Replanning parent.
fn close_replanning_plan(parent: &mut Plan, all_plans: &[Plan], event_tx: &broadcast::Sender<PlanStreamEvent>) -> anyhow::Result<()> {
    let child = all_plans
        .iter()
        .find(|p| p.parent_plan_id.as_deref() == Some(&parent.id));

    let Some(child) = child else {
        // Child not yet written to disk — wait for the next tick.
        return Ok(());
    };

    let terminal_status = match child.status {
        PlanStatus::Completed => Some(PlanStatus::Completed),
        PlanStatus::Failed => Some(PlanStatus::Failed),
        _ => None,
    };

    let Some(outcome) = terminal_status else {
        return Ok(()); // child still running
    };

    let duration = now_secs().saturating_sub(parent.created_at);
    parent.status = outcome.clone();
    parent.completed_at = Some(now_secs());

    let _ = append_event_for(ws(parent), &AuditEvent::PlanCompleted {
        plan_id: parent.id.clone(),
        outcome: format!("{outcome:?}"),
        total_duration_secs: duration,
        timestamp: now_secs(),
    });

    let ev = match &outcome {
        PlanStatus::Completed => PlanStreamEvent::PlanCompleted { plan_id: parent.id.clone() },
        _ => PlanStreamEvent::PlanFailed { plan_id: parent.id.clone() },
    };
    let _ = event_tx.send(ev);

    fire_plan_notification(&parent.intent, &parent.id, &outcome);

    info!(
        "parent plan {} closed as {outcome:?} (child: {})",
        parent.id, child.id
    );
    parent.save()?;
    Ok(())
}

fn advance_plan(plan: &mut Plan, event_tx: &broadcast::Sender<PlanStreamEvent>) -> anyhow::Result<()> {
    // Snapshot workspace before any mutable borrow of plan.nodes.
    let workspace = plan.scope.workspace.clone();
    let all_sessions = Session::load_all();

    // ── 0. Enforce plan-level timeout ─────────────────────────────────────────
    if let Some(max_secs) = plan.policy.max_duration_secs {
        let elapsed = now_secs().saturating_sub(plan.created_at);
        if elapsed >= max_secs {
            let reason = format!("plan timeout: {elapsed}s elapsed, limit {max_secs}s");
            warn!("{reason} — failing plan {}", plan.id);
            fail_plan_enforced(plan, &reason, event_tx)?;
            return Ok(());
        }
    }

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
        let plan_suffix = plan.id.trim_start_matches("plan_");
        let session_key = format!("orbit-plan-{plan_suffix}-{}", node.id);
        node.output_summary = capture_node_output(&session_key);
        node.token_usage = Some(estimate_token_usage(node));

        let outcome = verify_node(node, &CliBackend::new(node.engine));
        match outcome {
            VerifyOutcome::Pass => {
                node.status = NodeStatus::Completed;
                node.completed_at = Some(now_secs());
                let duration = node.started_at.map(|s| now_secs().saturating_sub(s)).unwrap_or(0);
                let _ = append_event_for(workspace.as_deref(), &AuditEvent::NodeCompleted {
                    plan_id: plan.id.clone(),
                    node_id: node.id.clone(),
                    duration_secs: duration,
                    timestamp: now_secs(),
                });
                let _ = event_tx.send(PlanStreamEvent::NodeCompleted {
                    plan_id: plan.id.clone(),
                    node_id: node.id.clone(),
                });
                run_hooks(
                    &HookEvent::PostNode,
                    &[
                        ("ORBIT_PLAN_ID", &plan.id),
                        ("ORBIT_NODE_ID", &node.id),
                        ("ORBIT_NODE_STATUS", "completed"),
                    ],
                );
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
                    let _ = append_event_for(workspace.as_deref(), &AuditEvent::NodeFailed {
                        plan_id: plan.id.clone(),
                        node_id: node.id.clone(),
                        reason: reason.clone(),
                        timestamp: now_secs(),
                    });
                    let _ = event_tx.send(PlanStreamEvent::NodeFailed {
                        plan_id: plan.id.clone(),
                        node_id: node.id.clone(),
                        error: reason.clone(),
                    });
                    run_hooks(
                        &HookEvent::PostNode,
                        &[
                            ("ORBIT_PLAN_ID", &plan.id),
                            ("ORBIT_NODE_ID", &node.id),
                            ("ORBIT_NODE_STATUS", "failed"),
                        ],
                    );
                    warn!("node {} failed in plan {}: {reason}", node.id, plan.id);
                }
            }
        }
    }

    // ── 2. Dispatch ready Pending nodes ───────────────────────────────────────

    // Budget hard-stop: fail remaining work if any budget limit is exhausted.
    if plan.is_budget_exhausted() {
        let spent: u64 = plan
            .nodes
            .iter()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.prompt_tokens + u.completion_tokens)
            .sum();
        let reason = format!(
            "budget exhausted: {spent} tokens spent, limit {}",
            plan.policy.max_tokens.unwrap_or(0)
        );
        warn!("{reason} — failing plan {}", plan.id);
        fail_plan_enforced(plan, &reason, event_tx)?;
        return Ok(());
    }

    if plan.is_cost_exhausted() {
        let spent: f64 = plan
            .nodes
            .iter()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.estimated_cost_usd)
            .sum();
        let reason = format!(
            "cost budget exhausted: ${spent:.4} spent, limit ${:.4}",
            plan.policy.max_cost_usd.unwrap_or(0.0)
        );
        warn!("{reason} — failing plan {}", plan.id);
        fail_plan_enforced(plan, &reason, event_tx)?;
        return Ok(());
    }

    if plan.is_nodes_exhausted() {
        let dispatched = plan.nodes.iter().filter(|n| !matches!(n.status, NodeStatus::Pending)).count();
        let reason = format!(
            "node budget exhausted: {dispatched} node(s) dispatched, limit {}",
            plan.policy.max_nodes.unwrap_or(0)
        );
        warn!("{reason} — failing plan {}", plan.id);
        fail_plan_enforced(plan, &reason, event_tx)?;
        return Ok(());
    }

    // Paused: skip dispatch but still process completions (step 1 already ran).
    if plan.status == PlanStatus::Paused {
        plan.save()?;
        return Ok(());
    }

    let ready_ids: Vec<String> = plan.ready_nodes().iter().map(|n| n.id.clone()).collect();

    for node_id in ready_ids {
        let Some(idx) = plan.nodes.iter().position(|n| n.id == node_id) else {
            continue;
        };

        // AwaitingApproval gate: block high-risk nodes that need human sign-off.
        // Skip the gate if the node was already explicitly approved (node.approved = true).
        let risk = plan.nodes[idx].policy.risk_level.clone();
        if !plan.nodes[idx].approved && plan.policy.require_approval_for.contains(&risk) {
            let node = &mut plan.nodes[idx];
            node.status = NodeStatus::AwaitingApproval;
            info!("node {node_id} requires approval ({risk:?}) in plan {}", plan.id);
            let _ = append_event_for(workspace.as_deref(), &AuditEvent::PolicyBlocked {
                plan_id: plan.id.clone(),
                node_id: node_id.clone(),
                reason: format!("{risk:?} risk requires approval"),
                timestamp: now_secs(),
            });
            continue;
        }

        let node_engine = plan.nodes[idx].engine;
        let node_intent = plan.nodes[idx].intent.clone();
        let node_label = plan.nodes[idx].label.clone();
        let dispatch_scope = plan.nodes[idx]
            .scope_override
            .clone()
            .unwrap_or_else(|| plan.scope.clone());

        let engine = selector::select(&plan.nodes[idx]).engine;
        let _ = node_engine;

        run_hooks(
            &HookEvent::PreNode,
            &[
                ("ORBIT_PLAN_ID", &plan.id),
                ("ORBIT_NODE_ID", &node_id),
                ("ORBIT_NODE_LABEL", &node_label),
            ],
        );

        let node_task_type = plan.nodes[idx].task_type.clone();
        match dispatch_node(&plan.id, &node_id, &node_label, &node_intent, &node_task_type, &dispatch_scope, engine) {
            Ok(session) => {
                let plan_suffix = plan.id.trim_start_matches("plan_");
                let session_key = format!("orbit-plan-{plan_suffix}-{node_id}");
                start_output_capture(&session_key, &session, &plan.id, &node_id, event_tx.clone());
                let node = &mut plan.nodes[idx];
                node.session_id = Some(session.id.clone());
                node.status = NodeStatus::Running;
                node.started_at = Some(now_secs());
                info!("dispatched node {} → session {} in plan {}", node_id, session.id, plan.id);
                let _ = append_event_for(workspace.as_deref(), &AuditEvent::NodeStarted {
                    plan_id: plan.id.clone(),
                    node_id: node_id.clone(),
                    engine: format!("{engine:?}"),
                    timestamp: now_secs(),
                });
                let _ = event_tx.send(PlanStreamEvent::NodeStarted {
                    plan_id: plan.id.clone(),
                    node_id: node_id.clone(),
                    label: node_label.clone(),
                });
            }
            Err(e) => {
                warn!("dispatch failed for node {node_id} in plan {}: {e}", plan.id);
                let node = &mut plan.nodes[idx];
                node.status = NodeStatus::Failed;
                node.error = Some(e.to_string());
                let _ = append_event_for(workspace.as_deref(), &AuditEvent::NodeFailed {
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
                        let _ = event_tx.send(PlanStreamEvent::PlanReplanning {
                            plan_id: plan.id.clone(),
                            child_plan_id: child_id.clone(),
                        });
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

        let _ = append_event_for(workspace.as_deref(), &AuditEvent::PlanCompleted {
            plan_id: plan.id.clone(),
            outcome: format!("{outcome:?}"),
            total_duration_secs: duration,
            timestamp: now_secs(),
        });

        let ev = match &outcome {
            PlanStatus::Completed => PlanStreamEvent::PlanCompleted { plan_id: plan.id.clone() },
            _ => PlanStreamEvent::PlanFailed { plan_id: plan.id.clone() },
        };
        let _ = event_tx.send(ev);

        fire_plan_notification(&plan.intent, &plan.id, &outcome);

        let total_cost: f64 = plan.nodes.iter()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.estimated_cost_usd)
            .sum();
        let total_tokens: u64 = plan.nodes.iter()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.prompt_tokens + u.completion_tokens)
            .sum();
        let _ = append_plan_run_for(workspace.as_deref(), &PlanRunRecord {
            plan_id: plan.id.clone(),
            intent: plan.intent.clone(),
            outcome: format!("{outcome:?}"),
            node_count: plan.nodes.len(),
            replan_count: plan.replan_count,
            duration_secs: duration,
            created_at: plan.created_at,
            scope_key: plan.scope.scope_key(),
            tags: vec![],
            cost_usd: total_cost,
            total_tokens,
            template_name: None,
        });

        let outcome_str = format!("{outcome:?}");
        run_hooks(
            &HookEvent::PostPlan,
            &[
                ("ORBIT_PLAN_ID", &plan.id),
                ("ORBIT_PLAN_OUTCOME", &outcome_str),
            ],
        );

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

    replanner::replan(plan, failed_node, reason, &recent_runs, &cfg, &CliBackend::new(replan_engine))
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

fn dispatch_node(
    plan_id: &str,
    node_id: &str,
    node_label: &str,
    node_intent: &str,
    task_type: &PlanNodeType,
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

    // Unique per-node session name doubles as the filesystem key for logs/intent files
    let plan_suffix = plan_id.trim_start_matches("plan_");
    let session_name = format!("orbit-plan-{plan_suffix}-{node_id}");

    // Specialist template injected before the intent (orbit context → specialist → intent)
    if let Some(template) = orbit_planner::templates::get_template(task_type) {
        let template_path = write_node_template(&session_name, template)?;
        merged.instructions.push(template_path);
    }

    let intent_path = write_node_intent(&session_name, node_label, node_intent)?;
    merged.instructions.push(intent_path);

    launcher::spawn_plan_node(&session_name, node_intent, &orbit_scope, &merged, engine)
}

// ── Output capture ────────────────────────────────────────────────────────────

/// Start piping tmux pane output to a per-node log file and stream lines
/// to the broadcast channel in real time.
fn start_output_capture(
    session_key: &str,
    session: &Session,
    plan_id: &str,
    node_id: &str,
    event_tx: broadcast::Sender<PlanStreamEvent>,
) {
    let Some(ref tmux_name) = session.tmux_session else {
        return;
    };
    let log_dir = std::env::temp_dir().join("orbit-plan-nodes");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("{session_key}.log"));

    // Pipe tmux pane output to the log file.
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

    // Tail the log file and emit NodeOutput events for each new line.
    let plan_id = plan_id.to_string();
    let node_id = node_id.to_string();
    std::thread::spawn(move || {
        // Wait for the file to appear (pipe-pane creates it on first output).
        let mut wait = 0u32;
        while !log_path.exists() && wait < 50 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            wait += 1;
        }
        if !log_path.exists() {
            return;
        }

        use std::io::{BufRead, BufReader};
        let Ok(file) = std::fs::File::open(&log_path) else {
            return;
        };
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        // Stop after 30 s of silence — the engine has finished by then.
        let mut empty_polls = 0u32;

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    empty_polls += 1;
                    if empty_polls > 300 {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Ok(_) => {
                    empty_polls = 0;
                    let content = line.trim_end_matches(['\n', '\r']).to_string();
                    if !content.is_empty() {
                        // A lagged or closed receiver is not an error — ignore.
                        let _ = event_tx.send(PlanStreamEvent::NodeOutput {
                            plan_id: plan_id.clone(),
                            node_id: node_id.clone(),
                            line: content,
                        });
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Read the last 100 lines from the node's log file.
fn capture_node_output(session_key: &str) -> Option<String> {
    let log_path = std::env::temp_dir()
        .join("orbit-plan-nodes")
        .join(format!("{session_key}.log"));
    let content = std::fs::read_to_string(&log_path).ok()?;
    if content.is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(100);
    Some(lines[start..].join("\n"))
}

fn write_node_template(session_key: &str, content: &str) -> anyhow::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("orbit-plan-nodes");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{session_key}.specialist.md"));
    std::fs::write(&path, content)?;
    Ok(path)
}

fn write_node_intent(session_key: &str, label: &str, intent: &str) -> anyhow::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("orbit-plan-nodes");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{session_key}.md"));
    let content = format!(
        "# Task: {label}\n\n{intent}\n\nComplete this task autonomously. When done, exit cleanly.\n"
    );
    std::fs::write(&path, &content)?;
    Ok(path)
}

// ── token usage estimation ────────────────────────────────────────────────────

/// Estimate token usage from intent and captured output text.
/// Engines run in headless `-p` mode and don't expose usage in stdout,
/// so we approximate: 1 token ≈ 4 characters, Claude Sonnet pricing ($3/$15 per MTok).
fn estimate_token_usage(node: &PlanNode) -> TokenUsage {
    let prompt_tokens = (node.intent.len() as u64).saturating_div(4).max(1);
    let completion_tokens = node
        .output_summary
        .as_deref()
        .map(|s| (s.len() as u64).saturating_div(4))
        .unwrap_or(0);
    let estimated_cost_usd = (prompt_tokens as f64 * 3.0 / 1_000_000.0)
        + (completion_tokens as f64 * 15.0 / 1_000_000.0);
    TokenUsage { prompt_tokens, completion_tokens, estimated_cost_usd }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Kill all Running nodes and mark the plan Failed with a policy reason.
fn fail_plan_enforced(
    plan: &mut Plan,
    reason: &str,
    event_tx: &broadcast::Sender<PlanStreamEvent>,
) -> anyhow::Result<()> {
    let workspace = plan.scope.workspace.clone();
    let all_sessions = Session::load_all();
    for node in plan.nodes.iter_mut() {
        match node.status {
            NodeStatus::Running => {
                // Kill the underlying tmux session.
                if let Some(ref sid) = node.session_id
                    && let Some(s) = all_sessions.iter().find(|s| &s.id == sid)
                    && let Some(ref tname) = s.tmux_session
                {
                    let _ = std::process::Command::new("tmux")
                        .args(["kill-session", "-t", tname])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
                node.status = NodeStatus::Failed;
                node.completed_at = Some(now_secs());
                node.error = Some(reason.to_string());
                let _ = append_event_for(workspace.as_deref(), &AuditEvent::NodeFailed {
                    plan_id: plan.id.clone(),
                    node_id: node.id.clone(),
                    reason: reason.to_string(),
                    timestamp: now_secs(),
                });
            }
            NodeStatus::Pending | NodeStatus::AwaitingApproval => {
                node.status = NodeStatus::Skipped;
            }
            _ => {}
        }
    }

    let duration = now_secs().saturating_sub(plan.created_at);
    plan.status = PlanStatus::Failed;
    plan.completed_at = Some(now_secs());

    let _ = append_event_for(workspace.as_deref(), &AuditEvent::PolicyBlocked {
        plan_id: plan.id.clone(),
        node_id: "plan".to_string(),
        reason: reason.to_string(),
        timestamp: now_secs(),
    });
    let _ = append_event_for(workspace.as_deref(), &AuditEvent::PlanCompleted {
        plan_id: plan.id.clone(),
        outcome: "Failed".to_string(),
        total_duration_secs: duration,
        timestamp: now_secs(),
    });
    let _ = event_tx.send(PlanStreamEvent::PlanFailed { plan_id: plan.id.clone() });

    fire_plan_notification(&plan.intent, &plan.id, &PlanStatus::Failed);

    plan.save()?;
    Ok(())
}

// ── notification ──────────────────────────────────────────────────────────────

fn fire_plan_notification(intent: &str, plan_id: &str, outcome: &PlanStatus) {
    let is_failure = !matches!(outcome, PlanStatus::Completed);
    let short_id = &plan_id[plan_id.len().saturating_sub(8)..];
    let short_intent: String = intent.chars().take(72).collect();
    let title = if is_failure { "orbit · Plan Failed" } else { "orbit · Plan Completed" };
    let body = format!("{short_intent}  [{short_id}]");
    let plan_id = plan_id.to_string();
    let intent = intent.to_string();
    std::thread::spawn(move || {
        orbit_core::notify::maybe_send(title, &body, is_failure);
        crate::webhook::maybe_fire(&plan_id, &intent, is_failure);
    });
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
