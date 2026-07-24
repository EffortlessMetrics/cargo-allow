//! Portable proof plan command transport (#2588-B).
//!
//! Provider-neutral argv plans for proof execution. Does not spawn processes,
//! resolve providers, or validate receipt semantics.

use serde::{Deserialize, Serialize};

pub const PROOF_PLAN_SCHEMA_ID: &str = "proof.plan.v1";
pub const PROOF_PLAN_COMMAND_SCHEMA_ID: &str = "proof.plan-command.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPlanCommandV1 {
    pub program: String,
    pub args: Vec<String>,
}

impl ProofPlanCommandV1 {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPlanV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub commands: Vec<ProofPlanCommandV1>,
}

impl ProofPlanV1 {
    pub fn new(plan_id: impl Into<String>, commands: Vec<ProofPlanCommandV1>) -> Self {
        Self {
            schema_id: PROOF_PLAN_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            commands,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofPlanError {
    InvalidSchemaId { observed: String },
    EmptyCommands,
    EmptyProgram { index: usize },
}

impl ProofPlanError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyCommands => "empty_commands",
            Self::EmptyProgram { .. } => "empty_program",
        }
    }
}

pub fn validate_proof_plan(plan: &ProofPlanV1) -> Result<(), ProofPlanError> {
    if plan.schema_id != PROOF_PLAN_SCHEMA_ID {
        return Err(ProofPlanError::InvalidSchemaId {
            observed: plan.schema_id.clone(),
        });
    }
    if plan.commands.is_empty() {
        return Err(ProofPlanError::EmptyCommands);
    }
    for (index, command) in plan.commands.iter().enumerate() {
        if command.program.trim().is_empty() {
            return Err(ProofPlanError::EmptyProgram { index });
        }
    }
    Ok(())
}

pub fn load_proof_plan_toml(text: &str) -> Result<ProofPlanV1, String> {
    toml::from_str(text).map_err(|err| err.to_string())
}
