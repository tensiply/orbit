use anyhow::Result;
use orbit_core::{
    memory::PlanRunRecord,
    plan::{Plan, PlanNode, PlanStatus},
};

use crate::backend::PlannerBackend;
use crate::planner::{invoke_planner, PlannerConfig};

// ── Public API ────────────────────────────────────────────────────────────────

/// Build the enhanced intent string that includes failure context for the new plan.
pub fn build_replan_intent(original_intent: &str, failed_node: &PlanNode, reason: &str) -> String {
    let output_ctx = failed_node.output_summary.as_deref().unwrap_or("(none)");
    let output_preview: String = output_ctx.chars().take(500).collect();
    format!(
        "{original_intent}\n\n\
         [PREVIOUS ATTEMPT FAILED]\n\
         Node '{}' failed: {reason}\n\
         Output summary: {output_preview}\n\
         Adjust the plan to address this failure.",
        failed_node.label
    )
}

/// Invoke the planner with failure context and return a new child plan ready to run.
pub fn replan(
    original: &Plan,
    failed_node: &PlanNode,
    reason: &str,
    recent_runs: &[PlanRunRecord],
    cfg: &PlannerConfig,
    backend: &dyn PlannerBackend,
) -> Result<Plan> {
    let enhanced_intent = build_replan_intent(&original.intent, failed_node, reason);
    let (mut child, _trace) =
        invoke_planner(&enhanced_intent, &original.scope, recent_runs, cfg, backend)?;
    child.parent_plan_id = Some(original.id.clone());
    child.replan_count = original.replan_count + 1;
    child.status = PlanStatus::Running;
    Ok(child)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::{
        engine::Engine,
        plan::{NodePolicy, NodeStatus, PlanNodeType, PlanScope},
    };

    fn make_failed_node() -> PlanNode {
        PlanNode {
            id: "n0".into(),
            task_type: PlanNodeType::Code,
            label: "implement feature".into(),
            intent: "write the code".into(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Failed,
            depends_on: vec![],
            policy: NodePolicy::default(),
            output_summary: Some("compilation failed".into()),
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: Some("verification failed".into()),
            retry_count: 1,
        }
    }

    #[test]
    fn replan_intent_contains_failure_context() {
        let node = make_failed_node();
        let intent = build_replan_intent("add unit tests", &node, "ExitCode failed");
        assert!(intent.contains("add unit tests"));
        assert!(intent.contains("[PREVIOUS ATTEMPT FAILED]"));
        assert!(intent.contains("implement feature"));
        assert!(intent.contains("ExitCode failed"));
        assert!(intent.contains("compilation failed"));
    }

    #[test]
    fn replan_intent_truncates_long_output() {
        let mut node = make_failed_node();
        node.output_summary = Some("x".repeat(2000));
        let intent = build_replan_intent("do work", &node, "reason");
        // Output preview is capped at 500 chars
        let prefix = "[PREVIOUS ATTEMPT FAILED]";
        let after_prefix = &intent[intent.find(prefix).unwrap()..];
        assert!(after_prefix.len() < 2000);
    }

    #[test]
    fn replan_intent_handles_no_output() {
        let mut node = make_failed_node();
        node.output_summary = None;
        let intent = build_replan_intent("do work", &node, "reason");
        assert!(intent.contains("(none)"));
    }

    #[test]
    fn replan_intent_empty_scope() {
        let node = make_failed_node();
        let _scope = PlanScope { workspace: None, tenant: None, project: None, repository: None };
        let intent = build_replan_intent("fix bug", &node, "test failed");
        assert!(intent.starts_with("fix bug"));
    }
}
