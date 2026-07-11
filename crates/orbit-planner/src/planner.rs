use anyhow::Result;
use orbit_core::{
    engine::Engine,
    ipc::PlannerTrace,
    memory::PlanRunRecord,
    plan::{
        NodePolicy, NodeStatus, Plan, PlanEdge, PlanNode, PlanNodeType, PlanScope, PlanStatus,
        RiskLevel, VerifyStrategy,
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::backend::PlannerBackend;

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are an expert software architect. Given a user intent and workspace scope, generate a Plan IR as JSON.

The Plan IR must follow this schema:
{
  "nodes": [
    {
      "id": "n0",
      "task_type": "Code",
      "label": "short description",
      "intent": "specific task for this node",
      "engine": "Claude",
      "depends_on": [],
      "scope_override": null,
      "policy": {
        "timeout_secs": null,
        "retry_max": 1,
        "risk_level": "Low",
        "verify": ["ExitCode"]
      }
    }
  ],
  "edges": []
}

task_type values: Code | Test | Review | Verify | Pr
engine values: Claude | Opencode | Gemini
risk_level values: Low | Medium | High
verify values: ExitCode | LlmJudge

scope_override: null means the node uses the plan's default scope. To target a different repo, set:
  "scope_override": { "workspace": "AI", "tenant": "AIDEV", "project": "AI-ECOSYSTEM", "repository": "orbit" }
If extra repos are listed in the context, use their exact field values.

Rules:
- For Phase 1, generate 1-3 nodes maximum.
- Use ExitCode as the default verify strategy.
- Respond ONLY with a JSON code block. No explanation.
"#;

// ── PlannerConfig ─────────────────────────────────────────────────────────────

pub struct PlannerConfig {
    pub engine: Engine,
    pub system_prompt_path: Option<PathBuf>,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            engine: Engine::Claude,
            system_prompt_path: None,
        }
    }
}

// ── Draft types for LLM response parsing ─────────────────────────────────────

#[derive(Deserialize)]
struct PlanDraft {
    nodes: Vec<NodeDraft>,
    #[serde(default)]
    edges: Vec<EdgeDraft>,
}

#[derive(Deserialize, Default)]
struct ScopeDraft {
    workspace: Option<String>,
    tenant: Option<String>,
    project: Option<String>,
    repository: Option<String>,
}

#[derive(Deserialize)]
struct NodeDraft {
    id: String,
    #[serde(default = "default_task_type")]
    task_type: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    intent: String,
    #[serde(default = "default_engine_str")]
    engine: String,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    scope_override: Option<ScopeDraft>,
    #[serde(default)]
    policy: NodePolicyDraft,
}

#[derive(Deserialize, Default)]
struct NodePolicyDraft {
    timeout_secs: Option<u64>,
    #[serde(default = "default_retry")]
    retry_max: u8,
    #[serde(default)]
    risk_level: String,
    #[serde(default)]
    verify: Vec<String>,
}

#[derive(Deserialize)]
struct EdgeDraft {
    from: String,
    to: String,
}

fn default_task_type() -> String {
    "Code".into()
}
fn default_engine_str() -> String {
    "Claude".into()
}
fn default_retry() -> u8 {
    1
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn user_prompt_path() -> PathBuf {
    let base = std::env::var("ORBIT_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            directories::BaseDirs::new()
                .map(|b| b.home_dir().join(".config"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        });
    base.join("orbit/planner.md")
}

pub fn build_system_prompt(cfg: &PlannerConfig) -> String {
    // Explicit override from config takes priority
    if let Some(ref path) = cfg.system_prompt_path
        && let Ok(text) = std::fs::read_to_string(path) {
            return text;
        }
    // User-editable fallback: ~/.config/orbit/planner.md
    let user_path = user_prompt_path();
    if user_path.exists()
        && let Ok(text) = std::fs::read_to_string(&user_path) {
            return text;
        }
    DEFAULT_SYSTEM_PROMPT.to_string()
}

pub fn create_plan_prompt(
    intent: &str,
    scope: &PlanScope,
    recent_runs: &[PlanRunRecord],
    extra_repos: &[orbit_core::plan::CrossRepoSpec],
) -> String {
    let mut prompt = format!("User intent: {intent}\n\nWorkspace scope:\n");
    if let Some(ref w) = scope.workspace {
        prompt.push_str(&format!("  workspace: {w}\n"));
    }
    if let Some(ref t) = scope.tenant {
        prompt.push_str(&format!("  tenant: {t}\n"));
    }
    if let Some(ref p) = scope.project {
        prompt.push_str(&format!("  project: {p}\n"));
    }
    if let Some(ref r) = scope.repository {
        prompt.push_str(&format!("  repository: {r}\n"));
    }

    if !extra_repos.is_empty() {
        prompt.push_str("\nAvailable repos for cross-repo nodes:\n");
        for repo in extra_repos {
            prompt.push_str(&format!("  alias: {}\n", repo.alias));
            if let Some(ref w) = repo.workspace { prompt.push_str(&format!("    workspace: {w}\n")); }
            if let Some(ref t) = repo.tenant { prompt.push_str(&format!("    tenant: {t}\n")); }
            if let Some(ref p) = repo.project { prompt.push_str(&format!("    project: {p}\n")); }
            if let Some(ref r) = repo.repository { prompt.push_str(&format!("    repository: {r}\n")); }
        }
    }

    let context: Vec<&PlanRunRecord> = recent_runs.iter().rev().take(3).collect();
    if !context.is_empty() {
        prompt.push_str("\nRecent plan runs (for context):\n");
        for run in context {
            prompt.push_str(&format!(
                "  - intent: \"{}\", outcome: {}, nodes: {}\n",
                run.intent, run.outcome, run.node_count
            ));
        }
    }

    prompt.push_str("\nGenerate the Plan IR JSON:\n");
    prompt
}

pub fn engine_cli_command(engine: &Engine) -> (&'static str, Vec<&'static str>) {
    match engine {
        Engine::Claude => ("claude", vec!["-p"]),
        Engine::Opencode => ("opencode", vec!["run"]),
        Engine::Gemini => ("gemini", vec!["-p"]),
    }
}

fn hash_prompt(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn parse_task_type(s: &str) -> PlanNodeType {
    match s {
        "Test" => PlanNodeType::Test,
        "Review" => PlanNodeType::Review,
        "Verify" => PlanNodeType::Verify,
        "Pr" | "PR" => PlanNodeType::Pr,
        "Code" => PlanNodeType::Code,
        other => PlanNodeType::Custom(other.to_string()),
    }
}

fn parse_risk_level(s: &str) -> RiskLevel {
    match s {
        "Medium" => RiskLevel::Medium,
        "High" => RiskLevel::High,
        _ => RiskLevel::Low,
    }
}

fn parse_verify(v: &[String]) -> Vec<VerifyStrategy> {
    v.iter()
        .map(|s| match s.as_str() {
            "LlmJudge" => VerifyStrategy::LlmJudge,
            _ => VerifyStrategy::ExitCode,
        })
        .collect()
}

fn draft_to_node(d: NodeDraft) -> PlanNode {
    let engine = d.engine.parse::<Engine>().unwrap_or(Engine::Claude);
    let verify = if d.policy.verify.is_empty() {
        vec![VerifyStrategy::ExitCode]
    } else {
        parse_verify(&d.policy.verify)
    };
    PlanNode {
        id: d.id,
        task_type: parse_task_type(&d.task_type),
        label: d.label,
        intent: d.intent,
        engine,
        scope_override: d.scope_override.map(|s| PlanScope {
            workspace: s.workspace,
            tenant: s.tenant,
            project: s.project,
            repository: s.repository,
        }),
        status: NodeStatus::Pending,
        depends_on: d.depends_on,
        policy: NodePolicy {
            timeout_secs: d.policy.timeout_secs,
            retry_max: d.policy.retry_max,
            risk_level: parse_risk_level(&d.policy.risk_level),
            verify,
        },
        output_summary: None,
        session_id: None,
        token_usage: None,
        started_at: None,
        completed_at: None,
        error: None,
        retry_count: 0,
    }
}

fn fallback_single_node(intent: &str, engine: Engine) -> PlanNode {
    PlanNode {
        id: "n0".into(),
        task_type: PlanNodeType::Code,
        label: "execute intent".into(),
        intent: intent.to_string(),
        engine,
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
    }
}

fn parse_llm_response(
    raw: &str,
    intent: &str,
    scope: &PlanScope,
    cfg: &PlannerConfig,
    system_prompt: &str,
) -> Result<Plan> {
    let json_str = extract_json_str(raw);

    let mut plan = Plan::new(intent, scope.clone(), cfg.engine);
    plan.status = PlanStatus::Planning;
    plan.planner_prompt_hash = hash_prompt(system_prompt);

    // Try PlanDraft (nodes + edges)
    if let Ok(draft) = serde_json::from_str::<PlanDraft>(json_str) {
        plan.nodes = draft.nodes.into_iter().map(draft_to_node).collect();
        plan.edges = draft
            .edges
            .into_iter()
            .map(|e| PlanEdge { from: e.from, to: e.to })
            .collect();
        return Ok(plan);
    }

    // Try bare array of NodeDraft
    if let Ok(nodes) = serde_json::from_str::<Vec<NodeDraft>>(json_str) {
        plan.nodes = nodes.into_iter().map(draft_to_node).collect();
        return Ok(plan);
    }

    // Fallback: single-node plan
    tracing::warn!("planner response unparseable, using fallback single-node plan");
    plan.nodes = vec![fallback_single_node(intent, cfg.engine)];
    Ok(plan)
}

// ── Scope suggestion ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ScopeSuggestion {
    workspace: Option<String>,
    tenant: Option<String>,
    project: Option<String>,
    repository: Option<String>,
}

/// Ask the engine to suggest an Orbit scope based on the cwd path and intent.
/// Returns `None` if the engine call fails or the response is unparseable.
pub fn suggest_scope(
    cwd: &std::path::Path,
    intent: &str,
    backend: &dyn PlannerBackend,
) -> Option<(Option<String>, Option<String>, Option<String>, Option<String>)> {
    let prompt = format!(
        r#"You are helping identify the workspace scope for a coding task in the Orbit CLI.

Current directory: {cwd}
Intent: "{intent}"

The Orbit scope hierarchy maps to a directory structure like:
  ~/WORKSPACE/TENANT/PROJECT/REPOSITORY

Based ONLY on the directory path provided, suggest the most likely scope fields.
Return ONLY valid JSON (no explanation, no markdown fences), like:
{{"workspace":"AI","tenant":"AIDEV","project":"AI-ECOSYSTEM","repository":"orbit"}}

Use null for any field you cannot determine from the path alone."#,
        cwd = cwd.display()
    );

    let raw = backend.call(&prompt).ok()?;
    let json_str = extract_json_str(&raw);
    let suggestion: ScopeSuggestion = serde_json::from_str(json_str).ok()?;
    Some((suggestion.workspace, suggestion.tenant, suggestion.project, suggestion.repository))
}

fn extract_json_str(raw: &str) -> &str {
    if let Some(start) = raw.find("```json") {
        let after = &raw[start + 7..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim();
    }
    if let Some(start) = raw.find("```") {
        let after = &raw[start + 3..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim();
    }
    raw.trim()
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn invoke_planner(
    intent: &str,
    scope: &PlanScope,
    recent_runs: &[PlanRunRecord],
    cfg: &PlannerConfig,
    backend: &dyn PlannerBackend,
    extra_repos: &[orbit_core::plan::CrossRepoSpec],
) -> Result<(Plan, PlannerTrace)> {
    let system_prompt = build_system_prompt(cfg);
    let user_prompt = create_plan_prompt(intent, scope, recent_runs, extra_repos);
    let full_prompt = format!("{system_prompt}\n\n---\n\n{user_prompt}");

    let raw = backend.call(&full_prompt)?;
    let plan = parse_llm_response(&raw, intent, scope, cfg, &system_prompt)?;
    let trace = PlannerTrace { system_prompt, user_prompt, raw_response: raw };
    Ok((plan, trace))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scope() -> PlanScope {
        PlanScope {
            workspace: Some("AI".into()),
            tenant: Some("AIDEV".into()),
            project: None,
            repository: None,
        }
    }

    #[test]
    fn parse_valid_json_response() {
        let raw = r#"```json
{
  "nodes": [
    {
      "id": "n0",
      "task_type": "Code",
      "label": "implement feature",
      "intent": "add the feature",
      "engine": "Claude",
      "depends_on": [],
      "policy": { "retry_max": 1, "risk_level": "Low", "verify": ["ExitCode"] }
    }
  ],
  "edges": []
}
```"#;
        let cfg = PlannerConfig::default();
        let scope = test_scope();
        let plan = parse_llm_response(raw, "do stuff", &scope, &cfg, "sys").unwrap();
        assert_eq!(plan.nodes.len(), 1);
        assert_eq!(plan.nodes[0].id, "n0");
        assert_eq!(plan.nodes[0].engine, Engine::Claude);
    }

    #[test]
    fn fallback_on_unparseable_response() {
        let raw = "Sorry, I cannot help with that.";
        let cfg = PlannerConfig::default();
        let scope = test_scope();
        let plan = parse_llm_response(raw, "do stuff", &scope, &cfg, "sys").unwrap();
        assert_eq!(plan.nodes.len(), 1);
        assert_eq!(plan.nodes[0].id, "n0");
    }

    #[test]
    fn system_prompt_hash_is_set() {
        let raw = r#"{ "nodes": [], "edges": [] }"#;
        let cfg = PlannerConfig::default();
        let scope = test_scope();
        let plan = parse_llm_response(raw, "test", &scope, &cfg, "myprompt").unwrap();
        assert!(!plan.planner_prompt_hash.is_empty());
    }

    #[test]
    fn engine_cli_command_mapping() {
        assert_eq!(engine_cli_command(&Engine::Claude).0, "claude");
        assert_eq!(engine_cli_command(&Engine::Opencode).0, "opencode");
        assert_eq!(engine_cli_command(&Engine::Gemini).0, "gemini");
    }
}
