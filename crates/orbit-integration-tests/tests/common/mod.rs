use anyhow::Result;
use orbit_core::{
    engine::Engine,
    ipc::{Request, Response},
    plan::{
        NodePolicy, NodeStatus, Plan, PlanNode, PlanNodeType, PlanPolicy, PlanScope, PlanStatus,
    },
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use tokio::task::JoinHandle;

// ── TestHarness ───────────────────────────────────────────────────────────────

/// Starts an isolated daemon instance backed by a temporary directory.
///
/// Tests that use this harness MUST carry `#[serial]` from the `serial_test`
/// crate — the harness redirects `XDG_DATA_HOME` which is process-global.
pub struct TestHarness {
    pub dir: TempDir,
    pub sock: PathBuf,
    _server: JoinHandle<()>,
}

impl TestHarness {
    pub async fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let data_home = dir.path().join("data");
        std::fs::create_dir_all(&data_home).unwrap();

        // Redirect all XDG data paths to our temp dir.
        // SAFETY: tests are serialized via #[serial] — no concurrent env reads.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", &data_home);
        }

        let sock = dir.path().join("orbit-test.sock");
        let pid_file = dir.path().join("orbit-test.pid");
        let sock_clone = sock.clone();

        let server = tokio::spawn(async move {
            let _ = orbit_daemon::server::run_on(sock_clone, pid_file).await;
        });

        // Wait up to 2 s for the socket to appear.
        for _ in 0..100 {
            if sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(sock.exists(), "daemon socket did not appear within 2 s");

        TestHarness { dir, sock, _server: server }
    }

    pub async fn send(&self, req: &Request) -> Response {
        orbit_client::ipc::send_raw_to(&self.sock, req)
            .await
            .expect("IPC send failed")
    }

    pub async fn shutdown(&self) {
        let _ = self.send(&Request::Shutdown).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    pub fn sock(&self) -> &Path {
        &self.sock
    }

    /// Write a `Plan` JSON directly to the test data dir, bypassing the planner.
    pub fn write_plan(&self, plan: &Plan) -> Result<()> {
        let plans_dir = self.dir.path().join("data/orbit/plans");
        std::fs::create_dir_all(&plans_dir)?;
        let path = plans_dir.join(format!("{}.json", plan.id));
        std::fs::write(path, serde_json::to_string_pretty(plan)?)?;
        Ok(())
    }
}

// ── plan builders ─────────────────────────────────────────────────────────────

pub fn make_plan(id: &str, intent: &str, status: PlanStatus) -> Plan {
    Plan {
        id: id.to_string(),
        schema_version: 1,
        intent: intent.to_string(),
        scope: PlanScope { workspace: None, tenant: None, project: None, repository: None },
        nodes: vec![PlanNode {
            id: "n1".to_string(),
            task_type: PlanNodeType::Code,
            label: "step one".to_string(),
            intent: "do something".to_string(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Pending,
            depends_on: vec![],
            policy: NodePolicy::default(),
            output_summary: None,
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
        }],
        edges: vec![],
        status,
        policy: PlanPolicy::default(),
        created_at: 0,
        completed_at: None,
        parent_plan_id: None,
        replan_count: 0,
        planner_model: "test".to_string(),
        planner_prompt_hash: "0000".to_string(),
    }
}
