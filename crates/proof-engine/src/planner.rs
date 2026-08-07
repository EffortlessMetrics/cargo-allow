//! Proof planner over change obligation plans (#2589-A).

use crate::command_adapter::{
    compile_invocation_spec, default_cargo_allow_registry, validate_command_registry,
};
use proof_protocol::{ProofPlanCommandV1, ProofPlanV1, validate_proof_plan};

use crate::obligation_plan::{ChangeObligationPlanV1, validate_obligation_plan};
use crate::provider_registry::{ProviderRegistryV1, require_registered_provider};

pub const PROOF_PLANNER_SCHEMA_ID: &str = "proof.planner.v1";

const DEFAULT_COMMAND_ID: &str = "cargo-allow.check.no-new";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    ObligationPlan(crate::obligation_plan::ObligationPlanError),
    ProviderRegistry(crate::provider_registry::ProviderRegistryError),
    CommandRegistry(String),
    CommandSpec(String),
    NoCommandsPlanned,
}

impl PlannerError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ObligationPlan(_) => "obligation_plan_invalid",
            Self::ProviderRegistry(_) => "provider_registry_invalid",
            Self::CommandRegistry(_) => "command_registry_invalid",
            Self::CommandSpec(_) => "command_spec_invalid",
            Self::NoCommandsPlanned => "no_commands_planned",
        }
    }
}

pub fn plan_proof_execution(
    obligation_plan: &ChangeObligationPlanV1,
    provider_registry: &ProviderRegistryV1,
) -> Result<ProofPlanV1, PlannerError> {
    validate_obligation_plan(obligation_plan).map_err(PlannerError::ObligationPlan)?;
    let registry = default_cargo_allow_registry();
    validate_command_registry(&registry)
        .map_err(|err| PlannerError::CommandRegistry(err.as_str().to_string()))?;

    let mut commands = Vec::with_capacity(obligation_plan.obligations.len());
    for obligation in &obligation_plan.obligations {
        require_registered_provider(provider_registry, &obligation.provider_id)
            .map_err(PlannerError::ProviderRegistry)?;
        let plan_command = ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        );
        compile_invocation_spec(&registry, DEFAULT_COMMAND_ID, &plan_command)
            .map_err(|err| PlannerError::CommandSpec(err.as_str().to_string()))?;
        commands.push(plan_command);
    }

    if commands.is_empty() {
        return Err(PlannerError::NoCommandsPlanned);
    }

    let plan = ProofPlanV1::new(&obligation_plan.plan_id, commands);
    validate_proof_plan(&plan).map_err(|_| PlannerError::NoCommandsPlanned)?;
    Ok(plan)
}
