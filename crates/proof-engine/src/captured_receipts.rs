//! Captured receipt store for proof-engine orchestration (#2589-A).

use proof_protocol::{ProofReceiptError, ProofReceiptSetV1, validate_receipt_set};
use serde::{Deserialize, Serialize};

pub const CAPTURED_RECEIPT_STORE_SCHEMA_ID: &str = "proof.captured-receipt-store.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedReceiptStoreV1 {
    pub schema_id: String,
    pub sets: Vec<ProofReceiptSetV1>,
}

impl CapturedReceiptStoreV1 {
    pub fn new() -> Self {
        Self {
            schema_id: CAPTURED_RECEIPT_STORE_SCHEMA_ID.to_string(),
            sets: Vec::new(),
        }
    }

    pub fn capture(&mut self, set: ProofReceiptSetV1) -> Result<(), CapturedReceiptError> {
        validate_receipt_set(&set).map_err(CapturedReceiptError::Receipt)?;
        if self
            .sets
            .iter()
            .any(|existing| existing.plan_id == set.plan_id)
        {
            return Err(CapturedReceiptError::DuplicatePlanId {
                plan_id: set.plan_id.clone(),
            });
        }
        self.sets.push(set);
        Ok(())
    }

    pub fn get(&self, plan_id: &str) -> Option<&ProofReceiptSetV1> {
        self.sets.iter().find(|set| set.plan_id == plan_id)
    }
}

impl Default for CapturedReceiptStoreV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturedReceiptError {
    InvalidSchemaId { observed: String },
    Receipt(ProofReceiptError),
    DuplicatePlanId { plan_id: String },
}

impl CapturedReceiptError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::Receipt(_) => "receipt_invalid",
            Self::DuplicatePlanId { .. } => "duplicate_plan_id",
        }
    }
}

pub fn validate_captured_receipt_store(
    store: &CapturedReceiptStoreV1,
) -> Result<(), CapturedReceiptError> {
    if store.schema_id != CAPTURED_RECEIPT_STORE_SCHEMA_ID {
        return Err(CapturedReceiptError::InvalidSchemaId {
            observed: store.schema_id.clone(),
        });
    }
    for set in &store.sets {
        validate_receipt_set(set).map_err(CapturedReceiptError::Receipt)?;
    }
    Ok(())
}
