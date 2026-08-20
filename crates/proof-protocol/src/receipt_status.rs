//! Captured receipt manifest and proof-item status contracts (#3600).

use effortless_repo_protocol::{ANALYSIS_RECEIPT_SCHEMA_ID, AnalysisReceiptEnvelopeV1};
use serde::{Deserialize, Serialize};

pub const PROOF_RECEIPT_MANIFEST_SCHEMA_ID: &str = "proof.receipt-manifest.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofItemReceiptStatusV1 {
    SatisfiedByCurrentReceipt,
    CurrentFindings,
    CurrentFailed,
    CurrentPartial,
    CurrentUnsupported,
    CurrentNotProven,
    CurrentInstrumentFailure,
    ReceiptMissing,
    ReceiptMalformed,
    ReceiptStale,
    ReceiptForDifferentItem,
    ProviderUnavailable,
    ManualOrNativeOutstanding,
    NotApplicable,
    Conflict,
}

impl ProofItemReceiptStatusV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SatisfiedByCurrentReceipt => "satisfied_by_current_receipt",
            Self::CurrentFindings => "current_findings",
            Self::CurrentFailed => "current_failed",
            Self::CurrentPartial => "current_partial",
            Self::CurrentUnsupported => "current_unsupported",
            Self::CurrentNotProven => "current_not_proven",
            Self::CurrentInstrumentFailure => "current_instrument_failure",
            Self::ReceiptMissing => "receipt_missing",
            Self::ReceiptMalformed => "receipt_malformed",
            Self::ReceiptStale => "receipt_stale",
            Self::ReceiptForDifferentItem => "receipt_for_different_item",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ManualOrNativeOutstanding => "manual_or_native_outstanding",
            Self::NotApplicable => "not_applicable",
            Self::Conflict => "conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedReceiptManifestRowV1 {
    pub proof_item_id: String,
    pub plan_id: String,
    pub provider_id: String,
    pub capability_id: String,
    pub snapshot_identity: String,
    pub subject_identity: String,
    pub provider_request_identity: String,
    pub config_identity: String,
    pub receipt_generation: u32,
    pub receipt: AnalysisReceiptEnvelopeV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedReceiptManifestV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub rows: Vec<CapturedReceiptManifestRowV1>,
}

impl CapturedReceiptManifestV1 {
    pub fn new(plan_id: impl Into<String>, rows: Vec<CapturedReceiptManifestRowV1>) -> Self {
        Self {
            schema_id: PROOF_RECEIPT_MANIFEST_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            rows,
        }
    }
}

pub fn validate_captured_receipt_manifest(
    manifest: &CapturedReceiptManifestV1,
) -> Result<(), String> {
    if manifest.schema_id != PROOF_RECEIPT_MANIFEST_SCHEMA_ID {
        return Err(format!(
            "unexpected receipt manifest schema_id {}",
            manifest.schema_id
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for row in &manifest.rows {
        if row.proof_item_id.trim().is_empty()
            || row.plan_id.trim().is_empty()
            || row.provider_id.trim().is_empty()
            || row.capability_id.trim().is_empty()
            || row.snapshot_identity.trim().is_empty()
            || row.subject_identity.trim().is_empty()
            || row.provider_request_identity.trim().is_empty()
            || row.config_identity.trim().is_empty()
        {
            return Err("receipt manifest row contains a blank identity".to_string());
        }
        if row.plan_id != manifest.plan_id {
            return Err(format!(
                "receipt row {} belongs to plan {}, expected {}",
                row.proof_item_id, row.plan_id, manifest.plan_id
            ));
        }
        if row.receipt.schema_id != ANALYSIS_RECEIPT_SCHEMA_ID {
            return Err(format!(
                "receipt row {} has schema {}",
                row.proof_item_id, row.receipt.schema_id
            ));
        }
        if row.receipt.provider != row.provider_id {
            return Err(format!(
                "receipt row {} provider identity mismatch",
                row.proof_item_id
            ));
        }
        if !seen.insert(row.proof_item_id.as_str()) {
            return Err(format!("duplicate receipt row {}", row.proof_item_id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ProofItemReceiptStatusV1;

    #[test]
    fn status_tokens_are_stable_for_every_variant() -> Result<(), String> {
        let statuses = [
            ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt,
            ProofItemReceiptStatusV1::CurrentFindings,
            ProofItemReceiptStatusV1::CurrentFailed,
            ProofItemReceiptStatusV1::CurrentPartial,
            ProofItemReceiptStatusV1::CurrentUnsupported,
            ProofItemReceiptStatusV1::CurrentNotProven,
            ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            ProofItemReceiptStatusV1::ReceiptMissing,
            ProofItemReceiptStatusV1::ReceiptMalformed,
            ProofItemReceiptStatusV1::ReceiptStale,
            ProofItemReceiptStatusV1::ReceiptForDifferentItem,
            ProofItemReceiptStatusV1::ProviderUnavailable,
            ProofItemReceiptStatusV1::ManualOrNativeOutstanding,
            ProofItemReceiptStatusV1::NotApplicable,
            ProofItemReceiptStatusV1::Conflict,
        ];
        for status in statuses {
            if status.as_str().chars().any(char::is_uppercase) || status.as_str().is_empty() {
                return Err(format!("unstable status token for {status:?}"));
            }
        }
        Ok(())
    }
}
