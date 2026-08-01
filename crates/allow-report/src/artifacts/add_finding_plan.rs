use std::collections::BTreeMap;

use serde_json::Value;

use crate::{EvaluationContext, InventoryContext};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFindingPlanRepository {
    pub identity: String,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFindingPlanPolicy {
    pub path: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AddFindingPlanFinding {
    pub kind: String,
    pub family: Option<String>,
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub identity: BTreeMap<String, Value>,
    pub digest: String,
    pub source_file_digest: String,
    pub selector: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFindingPlanOutcome {
    pub status: String,
    pub allow_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFindingPlanCandidate {
    pub allow_id: String,
    pub mismatch_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFindingPlanProofPlan {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AddFindingPlanV1<'a> {
    pub tool_version: String,
    pub repository: AddFindingPlanRepository,
    pub inventory: InventoryContext<'a>,
    pub evaluation: EvaluationContext<'a>,
    pub inventory_basis_identity: String,
    pub policy: AddFindingPlanPolicy,
    pub finding: AddFindingPlanFinding,
    pub outcome: AddFindingPlanOutcome,
    pub candidates: Vec<AddFindingPlanCandidate>,
    pub required_fields: Vec<String>,
    pub proof_plans: Vec<AddFindingPlanProofPlan>,
}
