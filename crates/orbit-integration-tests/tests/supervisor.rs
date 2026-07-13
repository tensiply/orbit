mod common;

use common::{TestHarness, make_plan};
use orbit_core::{
    ipc::{PlanStreamEvent, Request, Response},
    plan::{NodePolicy, NodeStatus, PlanStatus, RiskLevel, TokenUsage},
};
use serial_test::serial;
use std::time::Duration;

// ── helpers ───────────────────────────────────────────────────────────────────

async fn wait_for_node_status(
    h: &TestHarness,
    plan_id: &str,
    node_id: &str,
    want: NodeStatus,
    timeout: Duration,
) -> NodeStatus {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let resp = h
            .send(&Request::GetPlan {
                id: plan_id.to_string(),
            })
            .await;
        if let Response::PlanInfo { plan } = resp
            && let Some(node) = plan.nodes.iter().find(|n| n.id == node_id)
            && node.status == want
        {
            return node.status.clone();
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Return actual status on timeout
    let resp = h
        .send(&Request::GetPlan {
            id: plan_id.to_string(),
        })
        .await;
    if let Response::PlanInfo { plan } = resp {
        plan.nodes
            .into_iter()
            .find(|n| n.id == node_id)
            .map(|n| n.status)
            .unwrap_or(NodeStatus::Failed)
    } else {
        NodeStatus::Failed
    }
}

async fn collect_terminal(
    mut rx: tokio::sync::mpsc::Receiver<PlanStreamEvent>,
    timeout: Duration,
) -> Vec<PlanStreamEvent> {
    let mut events = vec![];
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(ev)) => {
                let terminal = ev.is_terminal();
                events.push(ev);
                if terminal {
                    break;
                }
            }
            Ok(None) | Err(_) => break,
        }
    }
    events
}

// ── approval gate: IPC ───────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn approve_node_transitions_awaiting_to_pending() {
    let h = TestHarness::new().await;

    let mut plan = make_plan("plan_approve_basic", "review PR", PlanStatus::Running);
    plan.nodes[0].status = NodeStatus::AwaitingApproval;
    h.write_plan(&plan).unwrap();

    let resp = h
        .send(&Request::ApprovePlanNode {
            plan_id: plan.id.clone(),
            node_id: "n1".into(),
        })
        .await;
    assert!(
        matches!(resp, Response::PlanApproved { .. }),
        "expected PlanApproved, got {resp:?}"
    );

    let resp = h
        .send(&Request::GetPlan {
            id: plan.id.clone(),
        })
        .await;
    let Response::PlanInfo { plan: fetched } = resp else {
        panic!("expected PlanInfo, got {resp:?}");
    };
    assert_eq!(fetched.nodes[0].status, NodeStatus::Pending);
    assert!(fetched.nodes[0].approved, "approved flag must be set");

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn approve_node_not_awaiting_returns_error() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_approve_wrong_state", "refactor", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    // Node is Pending, not AwaitingApproval.
    let resp = h
        .send(&Request::ApprovePlanNode {
            plan_id: plan.id.clone(),
            node_id: "n1".into(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error for non-awaiting node, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn approve_nonexistent_plan_returns_error() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::ApprovePlanNode {
            plan_id: "ghost-plan".into(),
            node_id: "n1".into(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn approve_nonexistent_node_returns_error() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_approve_ghost_node", "deploy", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    let resp = h
        .send(&Request::ApprovePlanNode {
            plan_id: plan.id.clone(),
            node_id: "ghost-node".into(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error for unknown node, got {resp:?}"
    );

    h.shutdown().await;
}

// ── approval gate: supervisor tick ───────────────────────────────────────────

#[tokio::test]
#[serial]
async fn supervisor_gates_high_risk_node_to_awaiting_approval() {
    // Plan requires approval for High-risk nodes; the node's risk_level is High.
    // After one supervisor tick (≤100 ms in test mode) the node should be
    // AwaitingApproval.
    let h = TestHarness::new().await;

    let mut plan = make_plan(
        "plan_gate_high_risk",
        "drop table users",
        PlanStatus::Running,
    );
    plan.policy.require_approval_for = vec![RiskLevel::High];
    plan.nodes[0].policy = NodePolicy {
        risk_level: RiskLevel::High,
        ..NodePolicy::default()
    };
    h.write_plan(&plan).unwrap();

    let status = wait_for_node_status(
        &h,
        &plan.id,
        "n1",
        NodeStatus::AwaitingApproval,
        Duration::from_secs(2),
    )
    .await;
    assert_eq!(
        status,
        NodeStatus::AwaitingApproval,
        "high-risk node should be gated"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn approved_node_not_re_gated_by_supervisor() {
    // Regression: before the approved flag, the supervisor would re-gate
    // an approved node on every tick, making approvals ineffective.
    let h = TestHarness::new().await;

    let mut plan = make_plan(
        "plan_approve_no_regate",
        "run migration",
        PlanStatus::Running,
    );
    plan.policy.require_approval_for = vec![RiskLevel::High];
    plan.nodes[0].policy = NodePolicy {
        risk_level: RiskLevel::High,
        ..NodePolicy::default()
    };
    plan.nodes[0].status = NodeStatus::AwaitingApproval;
    h.write_plan(&plan).unwrap();

    // Approve the node.
    let resp = h
        .send(&Request::ApprovePlanNode {
            plan_id: plan.id.clone(),
            node_id: "n1".into(),
        })
        .await;
    assert!(matches!(resp, Response::PlanApproved { .. }));

    // Give the supervisor 3 ticks (300 ms) to process.
    tokio::time::sleep(Duration::from_millis(350)).await;

    let resp = h
        .send(&Request::GetPlan {
            id: plan.id.clone(),
        })
        .await;
    let Response::PlanInfo { plan: fetched } = resp else {
        panic!("expected PlanInfo");
    };
    // Node must NOT be AwaitingApproval again.
    assert_ne!(
        fetched.nodes[0].status,
        NodeStatus::AwaitingApproval,
        "approved node should not be re-gated"
    );

    h.shutdown().await;
}

// ── budget hard-stops (via supervisor streaming) ─────────────────────────────

#[tokio::test]
#[serial]
async fn stream_plan_fails_on_token_budget_exhausted() {
    let h = TestHarness::new().await;

    let mut plan = make_plan("plan_budget_tokens", "refactor", PlanStatus::Running);
    plan.policy.max_tokens = Some(100);
    // Node already consumed 200 tokens — over the limit.
    plan.nodes[0].token_usage = Some(TokenUsage {
        prompt_tokens: 150,
        completion_tokens: 50,
        estimated_cost_usd: 0.001,
    });
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .unwrap();
    let events = collect_terminal(rx, Duration::from_secs(3)).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlanStreamEvent::PlanFailed { .. })),
        "expected PlanFailed from token budget, got {events:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn stream_plan_fails_on_cost_budget_exhausted() {
    let h = TestHarness::new().await;

    let mut plan = make_plan("plan_budget_cost", "add feature", PlanStatus::Running);
    plan.policy.max_cost_usd = Some(0.001);
    // Node already cost $0.05 — over the $0.001 limit.
    plan.nodes[0].token_usage = Some(TokenUsage {
        prompt_tokens: 10_000,
        completion_tokens: 5_000,
        estimated_cost_usd: 0.05,
    });
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .unwrap();
    let events = collect_terminal(rx, Duration::from_secs(3)).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlanStreamEvent::PlanFailed { .. })),
        "expected PlanFailed from cost budget, got {events:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn stream_plan_fails_on_node_budget_exhausted() {
    let h = TestHarness::new().await;

    let mut plan = make_plan("plan_budget_nodes", "migrate db", PlanStatus::Running);
    plan.policy.max_nodes = Some(1);
    // The one node is already Running (dispatched count = 1 >= max_nodes = 1).
    // A second Pending node would be blocked.
    plan.nodes[0].status = NodeStatus::Running;
    // Add a second node that is still pending
    let mut node2 = plan.nodes[0].clone();
    node2.id = "n2".to_string();
    node2.status = NodeStatus::Pending;
    node2.session_id = None;
    plan.nodes.push(node2);
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .unwrap();
    let events = collect_terminal(rx, Duration::from_secs(3)).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlanStreamEvent::PlanFailed { .. })),
        "expected PlanFailed from node budget, got {events:?}"
    );

    h.shutdown().await;
}
