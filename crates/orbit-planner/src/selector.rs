use orbit_core::{engine::Engine, plan::{PlanNode, PlanNodeType}};

// ── DispatchConfig ────────────────────────────────────────────────────────────

pub struct DispatchConfig {
    pub engine: Engine,
}

/// Deterministic engine selection by node type.
/// Test → OpenCode (has terminal integration)
/// Review / Pr → Claude
/// Verify → Claude
/// Code → inherit from node's declared engine
pub fn select(node: &PlanNode) -> DispatchConfig {
    let engine = match node.task_type {
        PlanNodeType::Test => Engine::Opencode,
        PlanNodeType::Review | PlanNodeType::Pr | PlanNodeType::Verify => Engine::Claude,
        PlanNodeType::Code | PlanNodeType::Custom(_) => node.engine,
    };
    DispatchConfig { engine }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::plan::{NodePolicy, NodeStatus};

    fn make_node(task_type: PlanNodeType, engine: Engine) -> PlanNode {
        PlanNode {
            id: "n0".into(),
            task_type,
            label: "label".into(),
            intent: "intent".into(),
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

    #[test]
    fn test_node_uses_opencode() {
        let node = make_node(PlanNodeType::Test, Engine::Claude);
        let d = select(&node);
        assert_eq!(d.engine, Engine::Opencode);
    }

    #[test]
    fn review_node_uses_claude() {
        let node = make_node(PlanNodeType::Review, Engine::Gemini);
        let d = select(&node);
        assert_eq!(d.engine, Engine::Claude);
    }

    #[test]
    fn code_node_inherits_engine() {
        let node = make_node(PlanNodeType::Code, Engine::Gemini);
        let d = select(&node);
        assert_eq!(d.engine, Engine::Gemini);
    }
}
