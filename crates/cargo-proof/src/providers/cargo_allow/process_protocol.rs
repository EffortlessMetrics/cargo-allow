//! Public process protocol argv compilation (#2554).

use proof_engine::{
    DryRunCommandReportV1, ReviewedCommandEntryV1, ReviewedCommandRegistryV1,
    compile_invocation_spec, default_cargo_allow_registry, validate_command_registry,
};
use proof_protocol::{ProofPlanCommandV1, ProofPlanV1};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessProtocolError {
    RegistryInvalid,
    UnsupportedCommand { program: String },
    UnsupportedPlan { plan_id: String },
    EmptyPlan,
}

impl ProcessProtocolError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RegistryInvalid => "registry_invalid",
            Self::UnsupportedCommand { .. } => "unsupported_command",
            Self::UnsupportedPlan { .. } => "unsupported_plan",
            Self::EmptyPlan => "empty_plan",
        }
    }
}

pub fn validate_process_protocol_plan(plan: &ProofPlanV1) -> Result<(), ProcessProtocolError> {
    if plan.commands.is_empty() {
        return Err(ProcessProtocolError::EmptyPlan);
    }
    let registry = default_cargo_allow_registry();
    validate_command_registry(&registry).map_err(|_| ProcessProtocolError::RegistryInvalid)?;
    for command in &plan.commands {
        resolve_command_id(command, &registry)?;
    }
    Ok(())
}

pub fn compile_cargo_allow_dry_run(
    plan: &ProofPlanV1,
) -> Result<Vec<DryRunCommandReportV1>, ProcessProtocolError> {
    validate_process_protocol_plan(plan)?;
    let registry = default_cargo_allow_registry();
    let mut reports = Vec::with_capacity(plan.commands.len());
    for command in &plan.commands {
        let command_id = resolve_command_id(command, &registry)?;
        let spec = compile_invocation_spec(&registry, &command_id, command).map_err(|_| {
            ProcessProtocolError::UnsupportedCommand {
                program: command.program.clone(),
            }
        })?;
        reports.push(DryRunCommandReportV1::from_invocation_spec(&spec));
    }
    Ok(reports)
}

fn resolve_command_id(
    command: &ProofPlanCommandV1,
    registry: &ReviewedCommandRegistryV1,
) -> Result<String, ProcessProtocolError> {
    for entry in &registry.commands {
        if matches_plan_command(entry, command) {
            return Ok(entry.command_id.clone());
        }
    }
    Err(ProcessProtocolError::UnsupportedCommand {
        program: command.program.clone(),
    })
}

fn matches_plan_command(entry: &ReviewedCommandEntryV1, command: &ProofPlanCommandV1) -> bool {
    if entry.program != command.program {
        return false;
    }
    if command.args.len() < entry.argv_prefix.len() {
        return false;
    }
    entry
        .argv_prefix
        .iter()
        .enumerate()
        .all(|(index, prefix)| command.args.get(index) == Some(prefix))
}
