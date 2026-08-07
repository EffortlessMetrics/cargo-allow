//! Command receipt interpretation (#2603-B).
//!
//! Interprets proof-protocol receipt bindings against reviewed command specs.
//! Does not parse receipt payloads or authorize merge.

use effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID;
use proof_protocol::ProofReceiptBindingV1;

use crate::command_spec::CommandInvocationSpecV1;

pub const COMMAND_RECEIPT_OUTCOME_SCHEMA_ID: &str = "proof.command-receipt-outcome.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandReceiptStatusV1 {
    Bound,
    SchemaDrift,
    CommandMismatch,
}

impl CommandReceiptStatusV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::SchemaDrift => "schema_drift",
            Self::CommandMismatch => "command_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReceiptOutcomeV1 {
    pub schema_id: String,
    pub command_id: String,
    pub binding_id: String,
    pub status: CommandReceiptStatusV1,
    pub receipt_digest: String,
}

pub fn interpret_receipt_binding(
    spec: &CommandInvocationSpecV1,
    binding: &ProofReceiptBindingV1,
) -> CommandReceiptOutcomeV1 {
    let status = if binding.analysis_receipt_schema_id != ANALYSIS_RECEIPT_SCHEMA_ID {
        CommandReceiptStatusV1::SchemaDrift
    } else if !binding.binding_id.contains(spec.command_id.as_str()) {
        CommandReceiptStatusV1::CommandMismatch
    } else {
        CommandReceiptStatusV1::Bound
    };
    CommandReceiptOutcomeV1 {
        schema_id: COMMAND_RECEIPT_OUTCOME_SCHEMA_ID.to_string(),
        command_id: spec.command_id.clone(),
        binding_id: binding.binding_id.clone(),
        status,
        receipt_digest: binding.receipt_digest.clone(),
    }
}
