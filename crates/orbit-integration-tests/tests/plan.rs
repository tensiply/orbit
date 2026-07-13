mod common;

use common::{TestHarness, make_plan};
use orbit_core::{
    ipc::{Request, Response},
    plan::PlanStatus,
};
use serial_test::serial;

// ── list / get ────────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn list_plans_empty_initially() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::ListPlans { workspace_filter: None }).await;
    assert!(
        matches!(resp, Response::Plans { ref plans } if plans.is_empty()),
        "expected empty Plans, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn get_plan_not_found_returns_error() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::GetPlan { id: "does-not-exist".into() }).await;
    assert!(matches!(resp, Response::Error { .. }), "expected Error, got {resp:?}");

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn write_plan_then_get() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_test_get", "do a thing", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    let resp = h.send(&Request::GetPlan { id: plan.id.clone() }).await;
    assert!(
        matches!(&resp, Response::PlanInfo { plan: p } if p.id == plan.id),
        "expected PlanInfo, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn write_plan_then_list() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_test_list", "implement feature X", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    let resp = h.send(&Request::ListPlans { workspace_filter: None }).await;
    let Response::Plans { plans } = resp else {
        panic!("expected Plans, got {resp:?}");
    };
    assert!(plans.iter().any(|p| p.id == plan.id), "plan not found in list");

    h.shutdown().await;
}

// ── cancel ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn cancel_plan_changes_status_to_cancelled() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_test_cancel", "refactor auth", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    let resp = h.send(&Request::CancelPlan { id: plan.id.clone() }).await;
    assert!(
        matches!(resp, Response::PlanCancelled { .. }),
        "expected PlanCancelled, got {resp:?}"
    );

    let resp = h.send(&Request::GetPlan { id: plan.id.clone() }).await;
    let Response::PlanInfo { plan: fetched } = resp else {
        panic!("expected PlanInfo after cancel, got {resp:?}");
    };
    assert_eq!(fetched.status, PlanStatus::Cancelled, "plan should be Cancelled");

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn cancel_nonexistent_plan_returns_error() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::CancelPlan { id: "ghost-plan".into() }).await;
    assert!(matches!(resp, Response::Error { .. }), "expected Error, got {resp:?}");

    h.shutdown().await;
}

// ── pause / resume ────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn pause_then_resume_plan() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_test_pause", "add logging", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    let resp = h.send(&Request::PausePlan { id: plan.id.clone() }).await;
    assert!(matches!(resp, Response::PlanPaused { .. }), "expected PlanPaused, got {resp:?}");

    let resp = h.send(&Request::GetPlan { id: plan.id.clone() }).await;
    let Response::PlanInfo { plan: p } = resp else { panic!("expected PlanInfo") };
    assert_eq!(p.status, PlanStatus::Paused);

    let resp = h.send(&Request::ResumePlan { id: plan.id.clone() }).await;
    assert!(matches!(resp, Response::PlanResumed { .. }), "expected PlanResumed, got {resp:?}");

    let resp = h.send(&Request::GetPlan { id: plan.id.clone() }).await;
    let Response::PlanInfo { plan: p } = resp else { panic!("expected PlanInfo") };
    assert_eq!(p.status, PlanStatus::Running);

    h.shutdown().await;
}

// ── plan stats ────────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn get_plan_stats_returns_stats() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::GetPlanStats).await;
    assert!(
        matches!(resp, Response::PlanStats { .. }),
        "expected PlanStats, got {resp:?}"
    );

    h.shutdown().await;
}
