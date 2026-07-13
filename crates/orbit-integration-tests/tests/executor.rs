mod common;

use common::TestHarness;
use orbit_core::{
    engine::Engine,
    ipc::{Request, Response},
    plan::{
        NodePolicy, NodeStatus, Plan, PlanNode, PlanNodeType, PlanPolicy, PlanScope, PlanStatus,
    },
};
use serial_test::serial;
use std::{collections::HashMap, time::Duration};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_executor_plan(id: &str, executor: &str, executor_params: HashMap<String, String>) -> Plan {
    Plan {
        id: id.to_string(),
        schema_version: 1,
        intent: "run executor".to_string(),
        scope: PlanScope {
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
        },
        nodes: vec![PlanNode {
            id: "n1".to_string(),
            task_type: PlanNodeType::Custom("executor".to_string()),
            label: "executor node".to_string(),
            intent: "run the command".to_string(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Pending,
            depends_on: vec![],
            policy: NodePolicy {
                timeout_secs: Some(5),
                retry_max: 0,
                ..NodePolicy::default()
            },
            output_summary: None,
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
            approved: false,
            executor: Some(executor.to_string()),
            executor_params,
        }],
        edges: vec![],
        status: PlanStatus::Running,
        policy: PlanPolicy::default(),
        created_at: 0,
        completed_at: None,
        parent_plan_id: None,
        replan_count: 0,
        planner_model: "test".to_string(),
        planner_prompt_hash: "0000".to_string(),
    }
}

async fn wait_for_node_status(
    h: &TestHarness,
    plan_id: &str,
    node_id: &str,
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
            && !matches!(node.status, NodeStatus::Pending | NodeStatus::Running)
        {
            return node.status.clone();
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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

// ── shell executor: missing command param ─────────────────────────────────────

/// shell executor without the required `command` param → node fails with a
/// descriptive error rather than panicking.
#[tokio::test]
#[serial]
async fn shell_executor_missing_command_param_fails_node() {
    let h = TestHarness::new().await;

    let plan = make_executor_plan("plan_exec001", "shell", HashMap::new());
    h.write_plan(&plan).unwrap();

    // The supervisor scans disk on every 100ms tick — wait for dispatch + fail.
    let status = wait_for_node_status(&h, "plan_exec001", "n1", Duration::from_secs(3)).await;

    assert_eq!(
        status,
        NodeStatus::Failed,
        "expected Failed due to missing command param"
    );

    // Verify the error message is informative — only when GetPlan succeeds.
    let resp = h
        .send(&Request::GetPlan {
            id: "plan_exec001".to_string(),
        })
        .await;
    match resp {
        Response::PlanInfo { plan } => {
            let node = plan
                .nodes
                .iter()
                .find(|n| n.id == "n1")
                .expect("node n1 should exist");
            let err = node.error.as_deref().unwrap_or("");
            assert!(
                err.contains("command"),
                "error should mention 'command', got: {err}"
            );
        }
        other => panic!("expected PlanInfo, got {other:?}"),
    }

    h.shutdown().await;
}

// ── executor plugin not found ─────────────────────────────────────────────────

/// Referencing a non-existent plugin → node fails with a descriptive error.
#[tokio::test]
#[serial]
async fn unknown_executor_plugin_fails_node() {
    let h = TestHarness::new().await;

    let plan = make_executor_plan("plan_exec002", "nonexistent-plugin-xyz", HashMap::new());
    h.write_plan(&plan).unwrap();

    let status = wait_for_node_status(&h, "plan_exec002", "n1", Duration::from_secs(3)).await;
    assert_eq!(
        status,
        NodeStatus::Failed,
        "expected Failed for unknown plugin"
    );

    let resp = h
        .send(&Request::GetPlan {
            id: "plan_exec002".to_string(),
        })
        .await;
    match resp {
        Response::PlanInfo { plan } => {
            let node = plan
                .nodes
                .iter()
                .find(|n| n.id == "n1")
                .expect("node n1 should exist");
            let err = node.error.as_deref().unwrap_or("");
            assert!(
                err.contains("not found"),
                "error should mention 'not found', got: {err}"
            );
        }
        other => panic!("expected PlanInfo, got {other:?}"),
    }

    h.shutdown().await;
}

// ── shell executor: happy path (requires tmux) ────────────────────────────────

/// shell executor with `command = "true"` → node should complete successfully.
/// Skipped when tmux is not available.
#[tokio::test]
#[serial]
#[ignore = "requires tmux in the test environment"]
async fn shell_executor_echo_completes() {
    let h = TestHarness::new().await;

    let mut params = HashMap::new();
    params.insert("command".to_string(), "true".to_string());
    let plan = make_executor_plan("plan_exec003", "shell", params);
    h.write_plan(&plan).unwrap();

    let status = wait_for_node_status(&h, "plan_exec003", "n1", Duration::from_secs(10)).await;
    assert_eq!(status, NodeStatus::Completed);

    h.shutdown().await;
}

// ── unit: render_executor_command ─────────────────────────────────────────────
// These are re-verified here to confirm the built-in plugin TOMLs parse correctly.

#[test]
fn builtin_cargo_plugin_parses() {
    let plugins = orbit_core::plugin::load_all();
    let cargo = plugins.iter().find(|p| p.name == "cargo");
    assert!(cargo.is_some(), "cargo built-in plugin should be loaded");
    let cargo = cargo.unwrap();
    assert!(
        cargo.executor.is_some(),
        "cargo plugin should have an executor spec"
    );
}

#[test]
fn builtin_pytest_plugin_parses() {
    let plugins = orbit_core::plugin::load_all();
    let pytest = plugins.iter().find(|p| p.name == "pytest");
    assert!(pytest.is_some(), "pytest built-in plugin should be loaded");
    assert!(pytest.unwrap().executor.is_some());
}

#[test]
fn builtin_make_plugin_parses() {
    let plugins = orbit_core::plugin::load_all();
    let make = plugins.iter().find(|p| p.name == "make");
    assert!(make.is_some(), "make built-in plugin should be loaded");
    assert!(make.unwrap().executor.is_some());
}

#[test]
fn builtin_npm_plugin_parses() {
    let plugins = orbit_core::plugin::load_all();
    let npm = plugins.iter().find(|p| p.name == "npm");
    assert!(npm.is_some(), "npm built-in plugin should be loaded");
    assert!(npm.unwrap().executor.is_some());
}

#[test]
fn cargo_plugin_render_check() {
    let plugins = orbit_core::plugin::load_all();
    let cargo = plugins.iter().find(|p| p.name == "cargo").unwrap();
    let mut params = HashMap::new();
    params.insert("subcommand".to_string(), "check".to_string());
    let cmd = cargo.render_executor_command(&params).unwrap();
    assert!(cmd.contains(&"sh".to_string()));
    // The rendered sh -c command should contain "cargo check"
    let cmd_str = cmd.join(" ");
    assert!(
        cmd_str.contains("cargo"),
        "rendered command should contain 'cargo'"
    );
    assert!(
        cmd_str.contains("check"),
        "rendered command should contain 'check'"
    );
}

#[test]
fn cargo_plugin_requires_subcommand() {
    let plugins = orbit_core::plugin::load_all();
    let cargo = plugins.iter().find(|p| p.name == "cargo").unwrap();
    let err = cargo.render_executor_command(&HashMap::new()).unwrap_err();
    assert!(err.to_string().contains("subcommand"));
}
