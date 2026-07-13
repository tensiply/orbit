use orbit_core::{
    eval::{EvalCheck, EvalConstraint, EvalResult},
    plan::Plan,
};

/// Evaluate a plan against a set of structural constraints.
/// No engine is invoked — purely inspects the Plan IR.
pub fn eval(plan: &Plan, constraints: &[EvalConstraint]) -> EvalResult {
    let mut checks = Vec::with_capacity(constraints.len());

    for constraint in constraints {
        let check = match constraint {
            EvalConstraint::HasNodeType { node_type } => {
                let found = plan.nodes.iter().any(|n| &n.task_type == node_type);
                EvalCheck {
                    name: format!("has_node_type:{node_type:?}"),
                    passed: found,
                    detail: if found {
                        format!("plan contains a {node_type:?} node")
                    } else {
                        format!("plan missing required {node_type:?} node")
                    },
                }
            }

            EvalConstraint::MinNodes { count } => {
                let n = plan.nodes.len();
                EvalCheck {
                    name: format!("min_nodes:{count}"),
                    passed: n >= *count,
                    detail: format!("plan has {n} node(s) (min: {count})"),
                }
            }

            EvalConstraint::MaxNodes { count } => {
                let n = plan.nodes.len();
                EvalCheck {
                    name: format!("max_nodes:{count}"),
                    passed: n <= *count,
                    detail: format!("plan has {n} node(s) (max: {count})"),
                }
            }

            EvalConstraint::NodesHaveVerify => {
                let all = plan.nodes.iter().all(|n| !n.policy.verify.is_empty());
                EvalCheck {
                    name: "nodes_have_verify".into(),
                    passed: all,
                    detail: if all {
                        "all nodes have at least one verify strategy".into()
                    } else {
                        let missing = plan
                            .nodes
                            .iter()
                            .filter(|n| n.policy.verify.is_empty())
                            .map(|n| n.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("nodes without verify: {missing}")
                    },
                }
            }
        };
        checks.push(check);
    }

    let passed = checks.iter().all(|c| c.passed);
    EvalResult { passed, checks }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::{
        engine::Engine,
        plan::{
            NodePolicy, NodeStatus, Plan, PlanNode, PlanNodeType, PlanScope, PlanStatus,
            VerifyStrategy,
        },
    };

    fn make_plan(node_types: Vec<PlanNodeType>) -> Plan {
        let scope = PlanScope {
            workspace: Some("AI".into()),
            tenant: Some("AIDEV".into()),
            project: Some("AI-ECOSYSTEM".into()),
            repository: Some("orbit".into()),
        };
        let mut plan = Plan::new("test", scope, Engine::Claude);
        plan.status = PlanStatus::Running;
        for (i, ty) in node_types.into_iter().enumerate() {
            plan.nodes.push(PlanNode {
                id: format!("n{i}"),
                task_type: ty,
                label: format!("node {i}"),
                intent: "do it".into(),
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
            });
        }
        plan
    }

    #[test]
    fn has_node_type_pass() {
        let plan = make_plan(vec![PlanNodeType::Code, PlanNodeType::Test]);
        let result = eval(
            &plan,
            &[EvalConstraint::HasNodeType { node_type: PlanNodeType::Code }],
        );
        assert!(result.passed);
        assert!(result.checks[0].passed);
    }

    #[test]
    fn has_node_type_fail() {
        let plan = make_plan(vec![PlanNodeType::Code]);
        let result = eval(
            &plan,
            &[EvalConstraint::HasNodeType { node_type: PlanNodeType::Review }],
        );
        assert!(!result.passed);
    }

    #[test]
    fn min_max_nodes() {
        let plan = make_plan(vec![PlanNodeType::Code, PlanNodeType::Test]);

        let r = eval(&plan, &[EvalConstraint::MinNodes { count: 2 }]);
        assert!(r.passed);

        let r = eval(&plan, &[EvalConstraint::MinNodes { count: 3 }]);
        assert!(!r.passed);

        let r = eval(&plan, &[EvalConstraint::MaxNodes { count: 5 }]);
        assert!(r.passed);

        let r = eval(&plan, &[EvalConstraint::MaxNodes { count: 1 }]);
        assert!(!r.passed);
    }

    #[test]
    fn nodes_have_verify_pass() {
        let mut plan = make_plan(vec![PlanNodeType::Code]);
        plan.nodes[0].policy.verify = vec![VerifyStrategy::ExitCode];
        let r = eval(&plan, &[EvalConstraint::NodesHaveVerify]);
        assert!(r.passed);
    }

    #[test]
    fn nodes_have_verify_fail() {
        let plan = make_plan(vec![PlanNodeType::Code]); // default policy has verify: [ExitCode]
        // Override to empty
        let mut plan2 = plan;
        plan2.nodes[0].policy.verify = vec![];
        let r = eval(&plan2, &[EvalConstraint::NodesHaveVerify]);
        assert!(!r.passed);
        assert!(r.checks[0].detail.contains("n0"));
    }

    #[test]
    fn all_pass_when_constraints_empty() {
        let plan = make_plan(vec![]);
        let r = eval(&plan, &[]);
        assert!(r.passed);
        assert!(r.checks.is_empty());
    }
}
