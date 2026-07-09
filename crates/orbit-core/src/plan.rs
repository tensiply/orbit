use crate::engine::Engine;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn xdg_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

fn plans_dir() -> PathBuf {
    xdg_data_dir().join("orbit/plans")
}

fn gen_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut h);
    std::process::id().hash(&mut h);
    let val = h.finish();
    format!("plan_{:08x}", val & 0xFFFF_FFFF)
}

// ── enums ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanNodeType {
    Code,
    Test,
    Review,
    Verify,
    Pr,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    AwaitingApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    Planning,
    Running,
    Completed,
    Failed,
    Replanning,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VerifyStrategy {
    ExitCode,
    OutputContains { keywords: Vec<String> },
    LlmJudge,
    ShellCheck { command: Vec<String> },
}

// ── structs ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanScope {
    pub workspace: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
}

impl PlanScope {
    pub fn scope_key(&self) -> String {
        [
            self.workspace.as_deref().unwrap_or(""),
            self.tenant.as_deref().unwrap_or(""),
            self.project.as_deref().unwrap_or(""),
            self.repository.as_deref().unwrap_or(""),
        ]
        .join("/")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPolicy {
    pub max_tokens: Option<u64>,
    pub max_duration_secs: Option<u64>,
    pub max_replan_count: u8,
    pub require_approval_for: Vec<RiskLevel>,
}

impl Default for PlanPolicy {
    fn default() -> Self {
        Self {
            max_tokens: None,
            max_duration_secs: None,
            max_replan_count: 2,
            require_approval_for: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePolicy {
    pub timeout_secs: Option<u64>,
    pub retry_max: u8,
    pub risk_level: RiskLevel,
    pub verify: Vec<VerifyStrategy>,
}

impl Default for NodePolicy {
    fn default() -> Self {
        Self {
            timeout_secs: None,
            retry_max: 1,
            risk_level: RiskLevel::Low,
            verify: vec![VerifyStrategy::ExitCode],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: String,
    pub task_type: PlanNodeType,
    pub label: String,
    pub intent: String,
    pub engine: Engine,
    pub scope_override: Option<PlanScope>,
    pub status: NodeStatus,
    pub depends_on: Vec<String>,
    pub policy: NodePolicy,
    pub output_summary: Option<String>,
    pub session_id: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub error: Option<String>,
    #[serde(default)]
    pub retry_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub schema_version: u8,
    pub intent: String,
    pub scope: PlanScope,
    pub nodes: Vec<PlanNode>,
    pub edges: Vec<PlanEdge>,
    pub status: PlanStatus,
    pub policy: PlanPolicy,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub parent_plan_id: Option<String>,
    pub replan_count: u8,
    pub planner_model: String,
    pub planner_prompt_hash: String,
}

// ── impl Plan ─────────────────────────────────────────────────────────────────

impl Plan {
    pub fn new(intent: &str, scope: PlanScope, engine: Engine) -> Plan {
        Plan {
            id: gen_id(),
            schema_version: 0,
            intent: intent.to_string(),
            scope,
            nodes: vec![],
            edges: vec![],
            status: PlanStatus::Planning,
            policy: PlanPolicy::default(),
            created_at: now_secs(),
            completed_at: None,
            parent_plan_id: None,
            replan_count: 0,
            planner_model: engine.as_str().to_string(),
            planner_prompt_hash: String::new(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = plans_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", self.id));
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load(id: &str) -> Result<Plan> {
        let path = plans_dir().join(format!("{id}.json"));
        if !path.exists() {
            bail!("plan not found: {id}");
        }
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn load_all() -> Vec<Plan> {
        let dir = plans_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return vec![];
        };
        let mut plans: Vec<Plan> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| {
                fs::read_to_string(e.path())
                    .ok()
                    .and_then(|text| serde_json::from_str(&text).ok())
            })
            .collect();
        plans.sort_by_key(|p| p.created_at);
        plans
    }

    /// Nodes that are Pending with all dependencies Completed.
    pub fn ready_nodes(&self) -> Vec<&PlanNode> {
        let completed: std::collections::HashSet<&str> = self
            .nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Completed)
            .map(|n| n.id.as_str())
            .collect();
        self.nodes
            .iter()
            .filter(|n| {
                n.status == NodeStatus::Pending
                    && n.depends_on.iter().all(|d| completed.contains(d.as_str()))
            })
            .collect()
    }

    pub fn is_budget_exhausted(&self) -> bool {
        let Some(max) = self.policy.max_tokens else {
            return false;
        };
        let spent: u64 = self
            .nodes
            .iter()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.prompt_tokens + u.completion_tokens)
            .sum();
        spent >= max
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_scope() -> PlanScope {
        PlanScope {
            workspace: Some("AI".into()),
            tenant: Some("AIDEV".into()),
            project: Some("AI-ECOSYSTEM".into()),
            repository: Some("orbit".into()),
        }
    }

    #[test]
    fn plan_new_has_id() {
        let p = Plan::new("build something", make_scope(), Engine::Claude);
        assert!(p.id.starts_with("plan_"));
        assert_eq!(p.id.len(), 13); // "plan_" + 8 hex chars
    }

    #[test]
    fn save_and_load() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data").to_str().unwrap()); }

        let p = Plan::new("test intent", make_scope(), Engine::Claude);
        let id = p.id.clone();
        p.save().unwrap();

        let loaded = Plan::load(&id).unwrap();
        assert_eq!(loaded.intent, "test intent");
        assert_eq!(loaded.schema_version, 0);
    }

    #[test]
    fn ready_nodes_respects_deps() {
        let mut p = Plan::new("test", make_scope(), Engine::Claude);
        p.nodes.push(PlanNode {
            id: "n0".into(),
            task_type: PlanNodeType::Code,
            label: "step 1".into(),
            intent: "do step 1".into(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Completed,
            depends_on: vec![],
            policy: NodePolicy::default(),
            output_summary: None,
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
        });
        p.nodes.push(PlanNode {
            id: "n1".into(),
            task_type: PlanNodeType::Test,
            label: "step 2".into(),
            intent: "do step 2".into(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Pending,
            depends_on: vec!["n0".into()],
            policy: NodePolicy::default(),
            output_summary: None,
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
        });
        let ready = p.ready_nodes();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "n1");
    }

    #[test]
    fn scope_key_format() {
        let s = make_scope();
        assert_eq!(s.scope_key(), "AI/AIDEV/AI-ECOSYSTEM/orbit");
    }
}
