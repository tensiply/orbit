use orbit_core::plan::{Plan, PlanNode, RiskLevel};

// ── PolicyDecision ────────────────────────────────────────────────────────────

pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: Option<String>,
    pub require_approval: bool,
}

/// Static rules — no LLM involved.
pub fn evaluate(plan: &Plan, node: &PlanNode, budget_spent: u64) -> PolicyDecision {
    // Budget exhausted → block
    if let Some(max) = plan.policy.max_tokens
        && budget_spent >= max {
            return PolicyDecision {
                allowed: false,
                reason: Some(format!(
                    "token budget exhausted: {budget_spent} >= {max}"
                )),
                require_approval: false,
            };
        }

    // High-risk node with approval required for High
    let require_approval = node.policy.risk_level == RiskLevel::High
        && plan
            .policy
            .require_approval_for
            .contains(&RiskLevel::High);

    PolicyDecision {
        allowed: true,
        reason: None,
        require_approval,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::{
        engine::Engine,
        plan::{NodePolicy, NodeStatus, PlanNodeType, PlanPolicy, PlanScope, PlanStatus},
    };

    fn make_plan(max_tokens: Option<u64>, require_approval_for: Vec<RiskLevel>) -> Plan {
        Plan {
            id: "plan_test".into(),
            schema_version: 0,
            intent: "test".into(),
            scope: PlanScope {
                workspace: None,
                tenant: None,
                project: None,
                repository: None,
            },
            nodes: vec![],
            edges: vec![],
            status: PlanStatus::Running,
            policy: PlanPolicy {
                max_tokens,
                max_duration_secs: None,
                max_replan_count: 2,
                require_approval_for,
                max_cost_usd: None,
                max_nodes: None,
            },
            created_at: 0,
            completed_at: None,
            parent_plan_id: None,
            replan_count: 0,
            planner_model: "claude".into(),
            planner_prompt_hash: String::new(),
        }
    }

    fn make_node(risk_level: RiskLevel) -> PlanNode {
        PlanNode {
            id: "n0".into(),
            task_type: PlanNodeType::Code,
            label: "l".into(),
            intent: "i".into(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Pending,
            depends_on: vec![],
            policy: NodePolicy {
                timeout_secs: None,
                retry_max: 1,
                risk_level,
                verify: vec![],
            },
            output_summary: None,
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
            approved: false,
        }
    }

    #[test]
    fn budget_exhausted_blocks() {
        let plan = make_plan(Some(100), vec![]);
        let node = make_node(RiskLevel::Low);
        let d = evaluate(&plan, &node, 150);
        assert!(!d.allowed);
    }

    #[test]
    fn high_risk_requires_approval_when_configured() {
        let plan = make_plan(None, vec![RiskLevel::High]);
        let node = make_node(RiskLevel::High);
        let d = evaluate(&plan, &node, 0);
        assert!(d.allowed);
        assert!(d.require_approval);
    }

    #[test]
    fn low_risk_no_approval() {
        let plan = make_plan(None, vec![RiskLevel::High]);
        let node = make_node(RiskLevel::Low);
        let d = evaluate(&plan, &node, 0);
        assert!(d.allowed);
        assert!(!d.require_approval);
    }
}
