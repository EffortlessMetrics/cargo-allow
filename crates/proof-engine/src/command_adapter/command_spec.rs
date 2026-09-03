//! Structured command invocation specs (#2603-B).
//!
//! Compiles proof-plan argv into registry-bound invocation specs. Prose and
//! issue/spec text are rejected as executable authority.

use proof_protocol::ProofPlanCommandV1;
use serde::{Deserialize, Serialize};

use super::command_registry::{
    CommandRegistryError, ReviewedCommandEntryV1, ReviewedCommandRegistryV1,
    validate_command_registry,
};

pub const COMMAND_INVOCATION_SPEC_SCHEMA_ID: &str = "proof.command-invocation-spec.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandSourceKindV1 {
    ReviewedRegistry,
}

impl CommandSourceKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReviewedRegistry => "reviewed_registry",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandInvocationSpecV1 {
    pub schema_id: String,
    pub command_id: String,
    pub source_kind: CommandSourceKindV1,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd_policy: super::command_registry::CwdPolicyV1,
    pub env_allowlist: Vec<String>,
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub network: super::command_registry::NetworkAccessV1,
    pub timeout_ms: u64,
    pub cancellation: super::command_registry::CancellationPostureV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSpecError {
    Registry(CommandRegistryError),
    ProseNotExecutable,
    ProgramMismatch { expected: String, observed: String },
    ArgvPrefixMismatch { command_id: String },
    ArgvTrailingArgs { command_id: String },
}

impl CommandSpecError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Registry(_) => "registry_invalid",
            Self::ProseNotExecutable => "prose_not_executable",
            Self::ProgramMismatch { .. } => "program_mismatch",
            Self::ArgvPrefixMismatch { .. } => "argv_prefix_mismatch",
            Self::ArgvTrailingArgs { .. } => "argv_trailing_args",
        }
    }
}

pub fn reject_prose_as_executable(prose: &str) -> Result<(), CommandSpecError> {
    let trimmed = prose.trim();
    if trimmed.is_empty() {
        return Err(CommandSpecError::ProseNotExecutable);
    }
    if trimmed.contains("cargo-allow") || trimmed.contains("```") || trimmed.contains(" --") {
        return Err(CommandSpecError::ProseNotExecutable);
    }
    Ok(())
}

pub fn compile_invocation_spec(
    registry: &ReviewedCommandRegistryV1,
    command_id: &str,
    plan_command: &ProofPlanCommandV1,
) -> Result<CommandInvocationSpecV1, CommandSpecError> {
    validate_command_registry(registry).map_err(CommandSpecError::Registry)?;
    let Some(entry) = registry.find(command_id) else {
        return Err(CommandSpecError::Registry(
            CommandRegistryError::UnknownCommandId {
                command_id: command_id.to_string(),
            },
        ));
    };
    compile_invocation_from_entry(entry, plan_command)
}

fn compile_invocation_from_entry(
    entry: &ReviewedCommandEntryV1,
    plan_command: &ProofPlanCommandV1,
) -> Result<CommandInvocationSpecV1, CommandSpecError> {
    if entry.program != plan_command.program {
        return Err(CommandSpecError::ProgramMismatch {
            expected: entry.program.clone(),
            observed: plan_command.program.clone(),
        });
    }
    if plan_command.args.len() < entry.argv_prefix.len() {
        return Err(CommandSpecError::ArgvPrefixMismatch {
            command_id: entry.command_id.clone(),
        });
    }
    for (index, prefix) in entry.argv_prefix.iter().enumerate() {
        if plan_command.args.get(index) != Some(prefix) {
            return Err(CommandSpecError::ArgvPrefixMismatch {
                command_id: entry.command_id.clone(),
            });
        }
    }
    if !entry.allow_trailing_args && plan_command.args.len() != entry.argv_prefix.len() {
        return Err(CommandSpecError::ArgvTrailingArgs {
            command_id: entry.command_id.clone(),
        });
    }
    Ok(CommandInvocationSpecV1 {
        schema_id: COMMAND_INVOCATION_SPEC_SCHEMA_ID.to_string(),
        command_id: entry.command_id.clone(),
        source_kind: CommandSourceKindV1::ReviewedRegistry,
        program: entry.program.clone(),
        argv: plan_command.args.clone(),
        cwd_policy: entry.cwd_policy,
        env_allowlist: entry.env_allowlist.clone(),
        read_paths: entry.read_paths.clone(),
        write_paths: entry.write_paths.clone(),
        network: entry.network,
        timeout_ms: entry.timeout_ms,
        cancellation: entry.cancellation,
    })
}
