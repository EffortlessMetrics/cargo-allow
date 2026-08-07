//! Dry-run projection for proof plans (#2589-A).

use crate::command_adapter::{
    DryRunCommandReportV1, compile_invocation_spec, default_cargo_allow_registry,
    render_structured_argv, validate_command_registry,
};
use proof_protocol::{ProofPlanV1, validate_proof_plan};

pub const DRY_RUN_PLAN_REPORT_SCHEMA_ID: &str = "proof.dry-run-plan-report.v1";

const DEFAULT_COMMAND_ID: &str = "cargo-allow.check.no-new";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunPlanLineV1 {
    pub command_index: usize,
    pub structured_argv: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunPlanReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub lines: Vec<DryRunPlanLineV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryRunError {
    ProofPlan(String),
    CommandRegistry(String),
    CommandSpec(String),
}

impl DryRunError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ProofPlan(_) => "proof_plan_invalid",
            Self::CommandRegistry(_) => "command_registry_invalid",
            Self::CommandSpec(_) => "command_spec_invalid",
        }
    }
}

pub fn dry_run_proof_plan(plan: &ProofPlanV1) -> Result<DryRunPlanReportV1, DryRunError> {
    validate_proof_plan(plan).map_err(|err| DryRunError::ProofPlan(err.as_str().to_string()))?;
    let registry = default_cargo_allow_registry();
    validate_command_registry(&registry)
        .map_err(|err| DryRunError::CommandRegistry(err.as_str().to_string()))?;

    let mut lines = Vec::with_capacity(plan.commands.len());
    for (index, command) in plan.commands.iter().enumerate() {
        let invocation = compile_invocation_spec(&registry, DEFAULT_COMMAND_ID, command)
            .map_err(|err| DryRunError::CommandSpec(err.as_str().to_string()))?;
        let report = DryRunCommandReportV1::from_invocation_spec(&invocation);
        lines.push(DryRunPlanLineV1 {
            command_index: index,
            structured_argv: render_structured_argv(&report),
        });
    }

    Ok(DryRunPlanReportV1 {
        schema_id: DRY_RUN_PLAN_REPORT_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        lines,
    })
}
