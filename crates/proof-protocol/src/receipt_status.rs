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
    use super::{
        CapturedReceiptManifestRowV1, CapturedReceiptManifestV1, PROOF_RECEIPT_MANIFEST_SCHEMA_ID,
        ProofItemReceiptStatusV1,
    };
    use effortless_repo_protocol::{
        ANALYSIS_RECEIPT_SCHEMA_ID, AnalysisReceiptEnvelopeV1, ClaimBoundaryV1,
        RepositorySnapshotV1, ResolvedRevisionV1, ResultClassV1,
    };

    fn valid_manifest() -> CapturedReceiptManifestV1 {
        let snapshot = RepositorySnapshotV1::new_committed_head(
            "snapshot-1",
            "sha",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "commit".to_string(),
                tree: "tree".to_string(),
            },
        );
        CapturedReceiptManifestV1::new(
            "plan-1",
            vec![CapturedReceiptManifestRowV1 {
                proof_item_id: "item-1".to_string(),
                plan_id: "plan-1".to_string(),
                provider_id: "provider-1".to_string(),
                capability_id: "capability-1".to_string(),
                snapshot_identity: "snapshot-identity".to_string(),
                subject_identity: "subject-identity".to_string(),
                provider_request_identity: "request-identity".to_string(),
                config_identity: "config-identity".to_string(),
                receipt_generation: 1,
                receipt: AnalysisReceiptEnvelopeV1::new(
                    "provider-1",
                    snapshot,
                    ResultClassV1::Completed,
                    "provider.payload.v1",
                    serde_json::json!({}),
                    ClaimBoundaryV1::new("captured"),
                ),
            }],
        )
    }

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

    #[test]
    fn manifest_validation_rejects_each_structural_failure() -> Result<(), String> {
        let mut wrong_schema = valid_manifest();
        wrong_schema.schema_id = "wrong.schema.v1".to_string();
        if super::validate_captured_receipt_manifest(&wrong_schema).is_ok() {
            return Err("wrong manifest schema was accepted".to_string());
        }

        let mut blank_identity = valid_manifest();
        blank_identity
            .rows
            .first_mut()
            .ok_or_else(|| "blank fixture row missing".to_string())?
            .config_identity
            .clear();
        if super::validate_captured_receipt_manifest(&blank_identity).is_ok() {
            return Err("blank manifest identity was accepted".to_string());
        }

        let mut wrong_plan = valid_manifest();
        wrong_plan
            .rows
            .first_mut()
            .ok_or_else(|| "plan fixture row missing".to_string())?
            .plan_id = "other-plan".to_string();
        if super::validate_captured_receipt_manifest(&wrong_plan).is_ok() {
            return Err("wrong row plan was accepted".to_string());
        }

        let mut wrong_receipt_schema = valid_manifest();
        wrong_receipt_schema
            .rows
            .first_mut()
            .ok_or_else(|| "schema fixture row missing".to_string())?
            .receipt
            .schema_id = "wrong.receipt.v1".to_string();
        if super::validate_captured_receipt_manifest(&wrong_receipt_schema).is_ok() {
            return Err("wrong receipt schema was accepted".to_string());
        }

        let mut wrong_provider = valid_manifest();
        wrong_provider
            .rows
            .first_mut()
            .ok_or_else(|| "provider fixture row missing".to_string())?
            .receipt
            .provider = "other-provider".to_string();
        if super::validate_captured_receipt_manifest(&wrong_provider).is_ok() {
            return Err("wrong receipt provider was accepted".to_string());
        }

        let mut duplicate = valid_manifest();
        let duplicate_row = duplicate
            .rows
            .first()
            .cloned()
            .ok_or_else(|| "duplicate fixture row missing".to_string())?;
        duplicate.rows.push(duplicate_row);
        if super::validate_captured_receipt_manifest(&duplicate).is_ok() {
            return Err("duplicate receipt row was accepted".to_string());
        }
        if PROOF_RECEIPT_MANIFEST_SCHEMA_ID.is_empty() || ANALYSIS_RECEIPT_SCHEMA_ID.is_empty() {
            return Err("manifest schema constants must be nonempty".to_string());
        }
        Ok(())
    }
}
