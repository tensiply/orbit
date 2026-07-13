mod common;

use common::TestHarness;
use orbit_core::ipc::{Request, Response};
use serial_test::serial;

// ── lifecycle ─────────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn daemon_starts_and_accepts_connections() {
    let h = TestHarness::new().await;
    // If we got here, the socket appeared — connection works implicitly.
    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn daemon_responds_to_status() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::Status).await;
    assert!(
        matches!(resp, Response::Status { session_count: 0, .. }),
        "expected Status response, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn daemon_responds_to_health() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::Health).await;
    assert!(
        matches!(resp, Response::Health { running_plans: 0, .. }),
        "expected Health response, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn daemon_shutdown_returns_ok() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::Shutdown).await;
    assert!(matches!(resp, Response::Ok), "expected Ok, got {resp:?}");
}

#[tokio::test]
#[serial]
async fn daemon_list_sessions_empty() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::ListSessions).await;
    assert!(
        matches!(resp, Response::Sessions { ref sessions } if sessions.is_empty()),
        "expected empty Sessions, got {resp:?}"
    );

    h.shutdown().await;
}
