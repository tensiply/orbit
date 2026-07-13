mod common;

use common::{TestHarness, make_plan};
use orbit_core::{
    ipc::{Request, Response},
    plan::{NodeStatus, PlanStatus},
};
use serial_test::serial;

// ── schedule CRUD ─────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn list_schedules_empty_initially() {
    let h = TestHarness::new().await;

    let resp = h.send(&Request::ListSchedules).await;
    assert!(
        matches!(resp, Response::Schedules { ref schedules } if schedules.is_empty()),
        "expected empty Schedules, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn create_schedule_once_returns_created() {
    let h = TestHarness::new().await;

    let future_ts = 9_999_999_999u64; // year ~2286 — safely in the future
    let resp = h
        .send(&Request::CreateSchedule {
            intent: "deploy staging".into(),
            at: Some(future_ts),
            cron: None,
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;

    assert!(
        matches!(resp, Response::ScheduleCreated { .. }),
        "expected ScheduleCreated, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn create_schedule_cron_returns_created() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CreateSchedule {
            intent: "nightly lint check".into(),
            at: None,
            cron: Some("0 2 * * *".into()), // 02:00 every day
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;

    assert!(
        matches!(resp, Response::ScheduleCreated { .. }),
        "expected ScheduleCreated, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn create_schedule_without_at_or_cron_returns_error() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CreateSchedule {
            intent: "bad schedule".into(),
            at: None,
            cron: None,
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;

    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error for missing at/cron, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn create_schedule_invalid_cron_returns_error() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CreateSchedule {
            intent: "bad cron".into(),
            at: None,
            cron: Some("not a cron".into()),
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;

    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error for invalid cron, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn list_schedules_shows_created_schedule() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CreateSchedule {
            intent: "weekly report".into(),
            at: None,
            cron: Some("0 9 * * 1".into()), // Monday 09:00
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;
    let Response::ScheduleCreated { id, .. } = resp else {
        panic!("expected ScheduleCreated, got {resp:?}");
    };

    let resp = h.send(&Request::ListSchedules).await;
    let Response::Schedules { schedules } = resp else {
        panic!("expected Schedules, got {resp:?}");
    };
    assert!(
        schedules.iter().any(|s| s.id == id),
        "created schedule not found in list"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn cancel_schedule_removes_it_from_list() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CreateSchedule {
            intent: "hourly health check".into(),
            at: None,
            cron: Some("0 * * * *".into()),
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;
    let Response::ScheduleCreated { id, .. } = resp else {
        panic!("expected ScheduleCreated");
    };

    let resp = h.send(&Request::CancelSchedule { id: id.clone() }).await;
    assert!(
        matches!(resp, Response::ScheduleCancelled { .. }),
        "expected ScheduleCancelled, got {resp:?}"
    );

    let resp = h.send(&Request::ListSchedules).await;
    let Response::Schedules { schedules } = resp else {
        panic!("expected Schedules");
    };
    assert!(
        !schedules.iter().any(|s| s.id == id),
        "cancelled schedule should not appear in list"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn cancel_nonexistent_schedule_returns_error() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CancelSchedule {
            id: "ghost-schedule".into(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error, got {resp:?}"
    );

    h.shutdown().await;
}

// ── retry plan ────────────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn retry_failed_plan_resets_failed_nodes_to_pending() {
    let h = TestHarness::new().await;

    let mut plan = make_plan("plan_retry_failed", "run tests", PlanStatus::Failed);
    plan.nodes[0].status = NodeStatus::Failed;
    plan.nodes[0].error = Some("exit code 1".into());
    h.write_plan(&plan).unwrap();

    let resp = h
        .send(&Request::RetryPlan {
            id: plan.id.clone(),
        })
        .await;
    assert!(
        matches!(resp, Response::PlanRetried { reset_count: 1, .. }),
        "expected PlanRetried with reset_count=1, got {resp:?}"
    );

    let resp = h
        .send(&Request::GetPlan {
            id: plan.id.clone(),
        })
        .await;
    let Response::PlanInfo { plan: fetched } = resp else {
        panic!("expected PlanInfo");
    };
    assert_eq!(
        fetched.status,
        PlanStatus::Running,
        "retried plan should be Running"
    );
    assert_eq!(
        fetched.nodes[0].status,
        NodeStatus::Pending,
        "failed node should be reset to Pending"
    );
    assert!(
        fetched.nodes[0].error.is_none(),
        "error should be cleared after retry"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn retry_running_plan_returns_error() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_retry_running", "build", PlanStatus::Running);
    h.write_plan(&plan).unwrap();

    let resp = h
        .send(&Request::RetryPlan {
            id: plan.id.clone(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error for running plan, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn retry_completed_plan_returns_error() {
    let h = TestHarness::new().await;

    let plan = make_plan("plan_retry_completed", "deploy", PlanStatus::Completed);
    h.write_plan(&plan).unwrap();

    let resp = h
        .send(&Request::RetryPlan {
            id: plan.id.clone(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error for completed plan, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn retry_plan_with_no_failed_nodes_returns_error() {
    let h = TestHarness::new().await;

    // Cancelled plan with a Pending node — nothing to retry.
    let plan = make_plan("plan_retry_no_failed", "migrate", PlanStatus::Cancelled);
    h.write_plan(&plan).unwrap();

    let resp = h
        .send(&Request::RetryPlan {
            id: plan.id.clone(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error when no failed nodes, got {resp:?}"
    );

    h.shutdown().await;
}

#[tokio::test]
#[serial]
async fn retry_nonexistent_plan_returns_error() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::RetryPlan {
            id: "ghost-plan".into(),
        })
        .await;
    assert!(
        matches!(resp, Response::Error { .. }),
        "expected Error, got {resp:?}"
    );

    h.shutdown().await;
}

// ── engine-dependent operations (skipped in CI) ───────────────────────────────

/// RunScheduleNow calls invoke_planner which requires a real engine CLI.
/// Run manually with: cargo test -p orbit-integration-tests -- --ignored
#[tokio::test]
#[serial]
#[ignore = "requires real engine CLI (claude/opencode/gemini)"]
async fn run_schedule_now_fires_plan() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::CreateSchedule {
            intent: "quick smoke test".into(),
            at: Some(9_999_999_999),
            cron: None,
            repos: vec![],
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        })
        .await;
    let Response::ScheduleCreated { id, .. } = resp else {
        panic!("expected ScheduleCreated");
    };

    let resp = h.send(&Request::RunScheduleNow { id }).await;
    assert!(
        matches!(resp, Response::ScheduleFired { .. }),
        "expected ScheduleFired, got {resp:?}"
    );

    h.shutdown().await;
}

/// EvalPlan calls invoke_planner which requires a real engine CLI.
#[tokio::test]
#[serial]
#[ignore = "requires real engine CLI (claude/opencode/gemini)"]
async fn eval_plan_returns_result() {
    let h = TestHarness::new().await;

    let resp = h
        .send(&Request::EvalPlan {
            intent: "add a unit test for the parser module".into(),
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
            constraints: vec![],
        })
        .await;
    assert!(
        matches!(resp, Response::PlanEvalResult { .. }),
        "expected PlanEvalResult, got {resp:?}"
    );

    h.shutdown().await;
}
