use crate::plan::PlanNodeType;
use serde::{Deserialize, Serialize};

// ── constraints ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalConstraint {
    HasNodeType { node_type: PlanNodeType },
    MinNodes { count: usize },
    MaxNodes { count: usize },
    NodesHaveVerify,
}

// ── results ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub passed: bool,
    pub checks: Vec<EvalCheck>,
}
