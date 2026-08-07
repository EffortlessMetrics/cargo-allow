//! Proof receipt binding transport (#2588-B).
//!
//! Binds proof-plan commands to repo-protocol analysis receipt envelopes.
//! Does not parse receipt payloads or authorize merge.

use effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID;
use serde::{Deserialize, Serialize};

pub const PROOF_RECEIPT_BINDING_SCHEMA_ID: &str = "proof.receipt-binding.v1";
pub const PROOF_RECEIPT_SET_SCHEMA_ID: &str = "proof.receipt-set.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofReceiptBindingV1 {
    pub binding_id: String,
    pub plan_id: String,
    pub command_index: usize,
    pub analysis_receipt_schema_id: String,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofReceiptSetV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub bindings: Vec<ProofReceiptBindingV1>,
}

impl ProofReceiptSetV1 {
    pub fn new(plan_id: impl Into<String>, bindings: Vec<ProofReceiptBindingV1>) -> Self {
        Self {
            schema_id: PROOF_RECEIPT_SET_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            bindings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofReceiptError {
    InvalidSchemaId { observed: String },
    EmptyBindings,
    SchemaDrift { observed: String },
    PlanIdMismatch { expected: String, observed: String },
}

impl ProofReceiptError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyBindings => "empty_bindings",
            Self::SchemaDrift { .. } => "analysis_receipt_schema_drift",
            Self::PlanIdMismatch { .. } => "plan_id_mismatch",
        }
    }
}

pub fn validate_receipt_set(set: &ProofReceiptSetV1) -> Result<(), ProofReceiptError> {
    if set.schema_id != PROOF_RECEIPT_SET_SCHEMA_ID {
        return Err(ProofReceiptError::InvalidSchemaId {
            observed: set.schema_id.clone(),
        });
    }
    if set.bindings.is_empty() {
        return Err(ProofReceiptError::EmptyBindings);
    }
    for binding in &set.bindings {
        if binding.analysis_receipt_schema_id != ANALYSIS_RECEIPT_SCHEMA_ID {
            return Err(ProofReceiptError::SchemaDrift {
                observed: binding.analysis_receipt_schema_id.clone(),
            });
        }
        if binding.plan_id != set.plan_id {
            return Err(ProofReceiptError::PlanIdMismatch {
                expected: set.plan_id.clone(),
                observed: binding.plan_id.clone(),
            });
        }
    }
    Ok(())
}
