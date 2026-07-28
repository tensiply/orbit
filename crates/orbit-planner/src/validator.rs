use orbit_core::plan::{Plan, PlanNode, PlanScope, RiskLevel, ScopeInjection, VerifyStrategy};
use orbit_core::plugin::Plugin;
use std::collections::HashMap;

// ── ValidationContext ─────────────────────────────────────────────────────────

/// Catalog entry for an enabled MCP server.
#[derive(Debug, Clone)]
pub struct McpCatalogEntry {
    pub name: String,
}

pub struct ValidationContext<'a> {
    pub mcp_catalog: &'a [McpCatalogEntry],
    pub executor_catalog: &'a [Plugin],
    pub scope: &'a PlanScope,
}

// ── ValidationIssue ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Blocks execution — planner must fix before the plan can run.
    Error,
    /// Informational — plan can run but there may be a problem.
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub node_id: String,
    pub rule: String,
    pub message: String,
    pub severity: Severity,
}

// ── ValidationRule trait ──────────────────────────────────────────────────────

pub trait ValidationRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, node: &PlanNode, ctx: &ValidationContext<'_>) -> Option<ValidationIssue>;
}

// ── Built-in rules ────────────────────────────────────────────────────────────

struct RequiredFields;
impl ValidationRule for RequiredFields {
    fn name(&self) -> &'static str {
        "RequiredFields"
    }
    fn validate(&self, node: &PlanNode, _ctx: &ValidationContext<'_>) -> Option<ValidationIssue> {
        if node.label.trim().is_empty() || node.intent.trim().is_empty() {
            Some(ValidationIssue {
                node_id: node.id.clone(),
                rule: self.name().into(),
                message: "node must have non-empty 'label' and 'intent'".into(),
                severity: Severity::Error,
            })
        } else {
            None
        }
    }
}

struct McpServerExists;
impl ValidationRule for McpServerExists {
    fn name(&self) -> &'static str {
        "McpServerExists"
    }
    fn validate(&self, node: &PlanNode, ctx: &ValidationContext<'_>) -> Option<ValidationIssue> {
        if node.executor.as_deref() != Some("mcp") {
            return None;
        }
        let server = node.executor_params.get("server").map(|s| s.as_str()).unwrap_or("");
        if server.is_empty() || !ctx.mcp_catalog.iter().any(|e| e.name == server) {
            Some(ValidationIssue {
                node_id: node.id.clone(),
                rule: self.name().into(),
                message: format!(
                    "executor 'mcp' requires a 'server' param matching an enabled MCP server; got '{server}'"
                ),
                severity: Severity::Error,
            })
        } else {
            None
        }
    }
}

struct McpParamsValid;
impl ValidationRule for McpParamsValid {
    fn name(&self) -> &'static str {
        "McpParamsValid"
    }
    fn validate(&self, node: &PlanNode, _ctx: &ValidationContext<'_>) -> Option<ValidationIssue> {
        if node.executor.as_deref() != Some("mcp") {
            return None;
        }
        let has_server = node.executor_params.contains_key("server");
        let has_tool = node.executor_params.contains_key("tool");
        if !has_server || !has_tool {
            Some(ValidationIssue {
                node_id: node.id.clone(),
                rule: self.name().into(),
                message: "executor 'mcp' requires 'server' and 'tool' in executor_params".into(),
                severity: Severity::Error,
            })
        } else {
            None
        }
    }
}

struct ExecutorKnown;
impl ValidationRule for ExecutorKnown {
    fn name(&self) -> &'static str {
        "ExecutorKnown"
    }
    fn validate(&self, node: &PlanNode, ctx: &ValidationContext<'_>) -> Option<ValidationIssue> {
        let exec = node.executor.as_ref()?;
        // built-in executors that need no plugin registration
        if exec == "shell" || exec == "mcp" || exec == "orbit-context" {
            return None;
        }
        let known = ctx.executor_catalog.iter().any(|p| p.name == *exec);
        if !known {
            Some(ValidationIssue {
                node_id: node.id.clone(),
                rule: self.name().into(),
                message: format!(
                    "executor '{exec}' is not installed or not a known plugin; use 'shell' for arbitrary commands"
                ),
                severity: Severity::Error,
            })
        } else {
            None
        }
    }
}

struct RiskVerifyConsistency;
impl ValidationRule for RiskVerifyConsistency {
    fn name(&self) -> &'static str {
        "RiskVerifyConsistency"
    }
    fn validate(&self, node: &PlanNode, _ctx: &ValidationContext<'_>) -> Option<ValidationIssue> {
        if node.policy.risk_level == RiskLevel::High {
            let has_judge = node
                .policy
                .verify
                .iter()
                .any(|v| matches!(v, VerifyStrategy::LlmJudge));
            if !has_judge {
                return Some(ValidationIssue {
                    node_id: node.id.clone(),
                    rule: self.name().into(),
                    message: "high-risk node should include 'LlmJudge' in verify strategy for extra safety".into(),
                    severity: Severity::Warning,
                });
            }
        }
        None
    }
}

// ── DAG validator (plan-level) ────────────────────────────────────────────────

fn check_dag_acyclic(nodes: &[PlanNode]) -> Vec<ValidationIssue> {
    let id_to_idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut issues = Vec::new();

    // Simple DFS cycle detection
    let n = nodes.len();
    let mut color = vec![0u8; n]; // 0=white, 1=gray, 2=black

    fn dfs(
        u: usize,
        nodes: &[PlanNode],
        id_to_idx: &HashMap<&str, usize>,
        color: &mut Vec<u8>,
        issues: &mut Vec<ValidationIssue>,
    ) {
        color[u] = 1;
        for dep in &nodes[u].depends_on {
            if let Some(&v) = id_to_idx.get(dep.as_str()) {
                if color[v] == 1 {
                    issues.push(ValidationIssue {
                        node_id: nodes[u].id.clone(),
                        rule: "DagAcyclic".into(),
                        message: format!(
                            "dependency cycle detected: '{}' → '{}'",
                            nodes[u].id, dep
                        ),
                        severity: Severity::Error,
                    });
                } else if color[v] == 0 {
                    dfs(v, nodes, id_to_idx, color, issues);
                }
            } else {
                issues.push(ValidationIssue {
                    node_id: nodes[u].id.clone(),
                    rule: "DagAcyclic".into(),
                    message: format!("depends_on references unknown node id '{dep}'"),
                    severity: Severity::Error,
                });
            }
        }
        color[u] = 2;
    }

    for i in 0..n {
        if color[i] == 0 {
            dfs(i, nodes, &id_to_idx, &mut color, &mut issues);
        }
    }

    issues
}

// ── ValidationResult ──────────────────────────────────────────────────────────

pub struct ValidationResult {
    /// True if there are no Error-severity issues.
    pub is_valid: bool,
    pub issues: Vec<ValidationIssue>,
    /// Pre-formatted feedback prompt to feed into a planner retry.
    pub feedback_prompt: String,
}

fn build_feedback_prompt(issues: &[ValidationIssue]) -> String {
    let mut s = "The generated plan has validation errors that must be fixed:\n\n".to_string();
    let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
    for (idx, issue) in errors.iter().enumerate() {
        s.push_str(&format!(
            "{}. Node '{}' — [{}] {}\n",
            idx + 1,
            issue.node_id,
            issue.rule,
            issue.message
        ));
    }
    s.push_str("\nFix all errors above and regenerate the Plan IR JSON. Maintain the same overall structure.");
    s
}

// ── public API ────────────────────────────────────────────────────────────────

/// Built-in rule set applied to every generated plan.
pub fn default_rules() -> Vec<Box<dyn ValidationRule>> {
    vec![
        Box::new(RequiredFields),
        Box::new(McpServerExists),
        Box::new(McpParamsValid),
        Box::new(ExecutorKnown),
        Box::new(RiskVerifyConsistency),
    ]
}

/// Validate a plan against the provided rule set.
/// DAG acyclicity is always checked regardless of the rule set.
pub fn validate_plan(
    plan: &Plan,
    ctx: &ValidationContext<'_>,
    rules: &[Box<dyn ValidationRule>],
) -> ValidationResult {
    let mut issues = Vec::new();

    // Per-node rule checks
    for node in &plan.nodes {
        for rule in rules {
            if let Some(issue) = rule.validate(node, ctx) {
                issues.push(issue);
            }
        }
    }

    // Plan-level DAG check
    issues.extend(check_dag_acyclic(&plan.nodes));

    let has_errors = issues.iter().any(|i| i.severity == Severity::Error);
    let feedback_prompt = if has_errors {
        build_feedback_prompt(&issues)
    } else {
        String::new()
    };

    ValidationResult {
        is_valid: !has_errors,
        issues,
        feedback_prompt,
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Infer the ScopeInjection level from the node's executor type.
pub fn infer_scope_injection(executor: &Option<String>) -> ScopeInjection {
    match executor.as_deref() {
        None => ScopeInjection::Full,
        Some("mcp") => ScopeInjection::SingleMcp,
        Some("shell") | Some("orbit-context") => ScopeInjection::Minimal,
        Some(_) => ScopeInjection::Credentials,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::{
        engine::Engine,
        plan::{NodePolicy, NodeStatus, Plan, PlanNode, PlanNodeType, PlanPolicy, PlanScope, PlanStatus},
    };

    fn make_scope() -> PlanScope {
        PlanScope {
            workspace: Some("AI".into()),
            tenant: Some("AIDEV".into()),
            project: None,
            repository: None,
        }
    }

    fn empty_ctx() -> ValidationContext<'static> {
        ValidationContext {
            mcp_catalog: &[],
            executor_catalog: &[],
            scope: Box::leak(Box::new(make_scope())),
        }
    }

    fn make_node(id: &str, label: &str, executor: Option<&str>) -> PlanNode {
        PlanNode {
            id: id.into(),
            task_type: PlanNodeType::Code,
            label: label.into(),
            intent: "do something".into(),
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
            approved: false,
            executor: executor.map(|s| s.to_string()),
            executor_params: Default::default(),
            agent: None,
            scope_injection: Default::default(),
        }
    }

    fn make_plan(nodes: Vec<PlanNode>) -> Plan {
        Plan {
            id: "plan_test".into(),
            schema_version: 0,
            intent: "test".into(),
            scope: make_scope(),
            nodes,
            edges: vec![],
            status: PlanStatus::Planning,
            policy: PlanPolicy::default(),
            created_at: 0,
            completed_at: None,
            parent_plan_id: None,
            replan_count: 0,
            planner_model: "claude".into(),
            planner_prompt_hash: String::new(),
        }
    }

    #[test]
    fn required_fields_error_on_empty_label() {
        let mut node = make_node("n0", "", None);
        node.label = "".into();
        let plan = make_plan(vec![node]);
        let ctx = empty_ctx();
        let result = validate_plan(&plan, &ctx, &default_rules());
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.rule == "RequiredFields"));
    }

    #[test]
    fn mcp_params_error_when_missing_tool() {
        let mut node = make_node("n0", "call jira", Some("mcp"));
        node.executor_params.insert("server".into(), "jira".into());
        // 'tool' is missing → McpParamsValid should fire
        let plan = make_plan(vec![node]);
        let ctx = empty_ctx();
        let result = validate_plan(&plan, &ctx, &default_rules());
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.rule == "McpParamsValid"));
    }

    #[test]
    fn executor_known_error_on_unknown_plugin() {
        let node = make_node("n0", "run tests", Some("unknown-plugin-xyz"));
        let plan = make_plan(vec![node]);
        let ctx = empty_ctx();
        let result = validate_plan(&plan, &ctx, &default_rules());
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.rule == "ExecutorKnown"));
    }

    #[test]
    fn shell_executor_always_known() {
        let node = make_node("n0", "run script", Some("shell"));
        let plan = make_plan(vec![node]);
        let ctx = empty_ctx();
        let result = validate_plan(&plan, &ctx, &default_rules());
        assert!(result.is_valid, "shell executor should always be valid");
    }

    #[test]
    fn dag_cycle_detected() {
        let mut n0 = make_node("n0", "step 0", None);
        let mut n1 = make_node("n1", "step 1", None);
        n0.depends_on = vec!["n1".into()];
        n1.depends_on = vec!["n0".into()];
        let plan = make_plan(vec![n0, n1]);
        let ctx = empty_ctx();
        let result = validate_plan(&plan, &ctx, &default_rules());
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.rule == "DagAcyclic"));
    }

    #[test]
    fn unknown_dep_id_is_error() {
        let mut node = make_node("n0", "step", None);
        node.depends_on = vec!["n99".into()];
        let plan = make_plan(vec![node]);
        let ctx = empty_ctx();
        let result = validate_plan(&plan, &ctx, &default_rules());
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.rule == "DagAcyclic"));
    }

    #[test]
    fn infer_injection_levels() {
        assert_eq!(infer_scope_injection(&None), ScopeInjection::Full);
        assert_eq!(infer_scope_injection(&Some("mcp".into())), ScopeInjection::SingleMcp);
        assert_eq!(infer_scope_injection(&Some("shell".into())), ScopeInjection::Minimal);
        assert_eq!(infer_scope_injection(&Some("cargo".into())), ScopeInjection::Credentials);
    }
}
