mod common;

use common::{TestHarness, make_plan};
use orbit_core::{
    ipc::PlanStreamEvent,
    plan::PlanStatus,
};
use serial_test::serial;
use std::time::Duration;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Collect up to `limit` events from `rx`, stopping early on a terminal event
/// or after `timeout`. Returns the collected events.
async fn collect_events(
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

fn has_terminal(events: &[PlanStreamEvent]) -> bool {
    events.iter().any(|e| e.is_terminal())
}

// ── already-terminal plans ────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn stream_completed_plan_returns_immediate_event() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_stream_completed", "ship feature", PlanStatus::Completed);
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .expect("stream_plan_on failed");

    let events = collect_events(rx, Duration::from_secs(2)).await;

    assert!(
        events.iter().any(|e| matches!(e, PlanStreamEvent::PlanCompleted { .. })),
        "expected PlanCompleted in {events:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn stream_failed_plan_returns_immediate_event() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_stream_failed", "run tests", PlanStatus::Failed);
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .expect("stream_plan_on failed");

    let events = collect_events(rx, Duration::from_secs(2)).await;

    assert!(
        events.iter().any(|e| matches!(e, PlanStreamEvent::PlanFailed { .. })),
        "expected PlanFailed in {events:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn stream_cancelled_plan_returns_immediate_failed_event() {
    // Regression: Cancelled was not treated as terminal — subscribers would
    // block forever. Fixed: Cancelled now maps to PlanFailed immediately.
    let h = TestHarness::new().await;

    let plan = make_plan("plan_stream_cancelled", "deploy infra", PlanStatus::Cancelled);
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .expect("stream_plan_on failed");

    let events = collect_events(rx, Duration::from_secs(2)).await;

    assert!(
        events.iter().any(|e| matches!(e, PlanStreamEvent::PlanFailed { .. })),
        "expected PlanFailed (from Cancelled) in {events:?}"
    );

    h.shutdown().await;
}

// ── live plans driven by supervisor ──────────────────────────────────────────

#[tokio::test]
#[serial]
async fn stream_running_plan_fails_on_timeout_enforcement() {
    // Plan with max_duration_secs=1 and created_at=0 (Unix epoch) so
    // elapsed >> limit. The supervisor detects the timeout on its first
    // 100 ms tick and emits PlanFailed without dispatching any nodes.
    let h = TestHarness::new().await;

    let mut plan = make_plan("plan_stream_timeout", "fix bug", PlanStatus::Running);
    plan.policy.max_duration_secs = Some(1);
    plan.created_at = 0;
    h.write_plan(&plan).unwrap();

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .expect("stream_plan_on failed");

    let events = collect_events(rx, Duration::from_secs(3)).await;

    assert!(
        events.iter().any(|e| matches!(e, PlanStreamEvent::PlanFailed { .. })),
        "expected PlanFailed from timeout enforcement, got {events:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn cancel_running_plan_then_stream_returns_terminal() {
    // Cancel a running plan, then open a stream — should get PlanFailed
    // immediately because status is now Cancelled.
    let h = TestHarness::new().await;

    let plan = make_plan("plan_stream_cancel_then_stream", "audit logs", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    // Cancel it via IPC before opening the stream.
    use orbit_core::ipc::{Request, Response};
    let resp = h.send(&Request::CancelPlan { id: plan.id.clone() }).await;
    assert!(matches!(resp, Response::PlanCancelled { .. }));

    let rx = orbit_client::ipc::stream_plan_on(&plan.id, h.sock.clone())
        .await
        .expect("stream_plan_on failed");

    let events = collect_events(rx, Duration::from_secs(2)).await;

    assert!(has_terminal(&events), "expected terminal event after cancel, got {events:?}");

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn stream_unknown_plan_eventually_closes() {
    // Streaming an unknown plan ID should not block forever — the server
    // returns nothing and closes the connection when the daemon shuts down.
    let h = TestHarness::new().await;

    let rx = orbit_client::ipc::stream_plan_on("plan_does_not_exist", h.sock.clone())
        .await
        .expect("stream_plan_on failed");

    // Shut down the daemon, which should close the broadcast channel.
    h.shutdown().await;

    let events = collect_events(rx, Duration::from_secs(2)).await;
    // No panic is the assertion — the channel closes cleanly.
    let _ = events;
}
