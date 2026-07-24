//! Explicit execution gate for proof plans (#2589-A).
//!
//! Requires an explicit approval flag. Does not spawn processes.

use proof_protocol::{ProofPlanV1, validate_proof_plan};

pub const EXECUTION_GATE_SCHEMA_ID: &str = "proof.execution-gate.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionApprovalV1 {
    Denied,
    Explicit,
}

impl ExecutionApprovalV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::Explicit => "explicit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionGateReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub approval: ExecutionApprovalV1,
    pub would_execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    ProofPlan(String),
    ApprovalRequired,
}

impl ExecutionError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ProofPlan(_) => "proof_plan_invalid",
            Self::ApprovalRequired => "approval_required",
        }
    }
}

pub fn evaluate_execution_gate(
    plan: &ProofPlanV1,
    approval: ExecutionApprovalV1,
) -> Result<ExecutionGateReportV1, ExecutionError> {
    validate_proof_plan(plan).map_err(|err| ExecutionError::ProofPlan(err.as_str().to_string()))?;
    let would_execute = approval == ExecutionApprovalV1::Explicit;
    Ok(ExecutionGateReportV1 {
        schema_id: EXECUTION_GATE_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        approval,
        would_execute,
    })
}

pub fn require_explicit_execution(approval: ExecutionApprovalV1) -> Result<(), ExecutionError> {
    if approval == ExecutionApprovalV1::Explicit {
        Ok(())
    } else {
        Err(ExecutionError::ApprovalRequired)
    }
}
