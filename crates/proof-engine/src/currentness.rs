//! Currentness evaluation for captured receipts (#2589-A).
//!
//! Currentness vocabulary is owned by proof-protocol as `BindingCurrentnessV1`
//! (#3319 reconciliation). proof-engine previously declared a duplicate
//! `CurrentnessStatusV1` (Current/Stale/Missing) that was a strict subset of
//! the protocol's `BindingCurrentnessV1` (Current/Stale/Missing/Incomparable).
//! The engine now reuses the protocol type directly so there is a single
//! currentness vocabulary across the proof family.

use intent_protocol::IntentObligationPlanEnvelopeV1;
use proof_protocol::{BindingCurrentnessV1, ProofReceiptSetV1};

use crate::captured_receipts::{CapturedReceiptStoreV1, validate_captured_receipt_store};
use crate::intent_digest::intent_plan_identity;

pub const CURRENTNESS_REPORT_SCHEMA_ID: &str = "proof.currentness-report.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentnessReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub status: BindingCurrentnessV1,
    pub observed_digest: Option<String>,
    pub expected_digest: Option<String>,
}

pub fn evaluate_currentness(
    store: &CapturedReceiptStoreV1,
    plan_id: &str,
    expected_digest: Option<&str>,
) -> Result<CurrentnessReportV1, CurrentnessError> {
    validate_captured_receipt_store(store).map_err(CurrentnessError::CapturedReceipt)?;
    let Some(set) = store.get(plan_id) else {
        return Ok(CurrentnessReportV1 {
            schema_id: CURRENTNESS_REPORT_SCHEMA_ID.to_string(),
            plan_id: plan_id.to_string(),
            status: BindingCurrentnessV1::Missing,
            observed_digest: None,
            expected_digest: expected_digest.map(str::to_string),
        });
    };
    let observed_digest = receipt_set_digest(set);
    let status = match expected_digest {
        Some(expected) if expected == observed_digest => BindingCurrentnessV1::Current,
        Some(_) => BindingCurrentnessV1::Stale,
        None => BindingCurrentnessV1::Current,
    };
    Ok(CurrentnessReportV1 {
        schema_id: CURRENTNESS_REPORT_SCHEMA_ID.to_string(),
        plan_id: plan_id.to_string(),
        status,
        observed_digest: Some(observed_digest),
        expected_digest: expected_digest.map(str::to_string),
    })
}

pub fn receipt_set_digest(set: &ProofReceiptSetV1) -> String {
    let mut digests: Vec<&str> = set
        .bindings
        .iter()
        .map(|b| b.receipt_digest.as_str())
        .collect();
    digests.sort_unstable();
    digests.join("|")
}

/// Evaluate currentness for an intent obligation plan (#3316).
///
/// The lookup key is the intent-derived plan identity, which embeds the
/// content-complete intent plan digest. Receipts captured for one intent plan
/// therefore cannot validate a changed (stale or newer) intent plan: the
/// changed plan resolves to a distinct identity with no captured receipts and
/// is reported `Missing`, never `Current`.
pub fn evaluate_intent_plan_currentness(
    store: &CapturedReceiptStoreV1,
    envelope: &IntentObligationPlanEnvelopeV1,
) -> Result<CurrentnessReportV1, CurrentnessError> {
    let plan_id = intent_plan_identity(envelope).map_err(CurrentnessError::IntentDigest)?;
    evaluate_currentness(store, &plan_id, None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentnessError {
    CapturedReceipt(crate::captured_receipts::CapturedReceiptError),
    IntentDigest(String),
}

impl CurrentnessError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapturedReceipt(_) => "captured_receipt_invalid",
            Self::IntentDigest(_) => "intent_digest_failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::captured_receipts::CapturedReceiptStoreV1;
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPostureV1,
        IntentPhaseObligationKindV1, IntentPhaseObligationV1, RepositorySnapshotV1,
        ResolvedRevisionV1,
    };
    use proof_protocol::{ProofReceiptBindingV1, ProofReceiptSetV1};

    fn sample_identity() -> IntentIdentityEnvelopeV1 {
        IntentIdentityEnvelopeV1::new(
            RepositorySnapshotV1::new_committed_head(
                "identity",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "abc".to_string(),
                    tree: String::new(),
                },
            ),
            IntentArtifactKindV1::RequirementDocument,
            "test-artifact".to_string(),
            "test/source.md".to_string(),
            "test-content".to_string(),
        )
    }

    fn sample_envelope() -> IntentObligationPlanEnvelopeV1 {
        IntentObligationPlanEnvelopeV1::new(
            sample_identity(),
            "precommit",
            vec![IntentPhaseObligationV1 {
                obligation_id: "obl-1".to_string(),
                phase: "precommit".to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "Review evidence".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec!["doc:README.md".to_string()],
            }],
        )
    }

    fn receipt_set_for(envelope: &IntentObligationPlanEnvelopeV1) -> ProofReceiptSetV1 {
        let plan_id = intent_plan_identity(envelope).unwrap_or_default();
        ProofReceiptSetV1::new(
            plan_id.clone(),
            vec![ProofReceiptBindingV1 {
                binding_id: format!("{plan_id}#0"),
                plan_id,
                command_index: 0,
                analysis_receipt_schema_id: "repo.analysis-receipt.v1".to_string(),
                receipt_digest: "sha256:v1:binding".to_string(),
            }],
        )
    }

    #[test]
    fn stale_intent_plan_cannot_produce_current_proof_plan() -> Result<(), String> {
        let original = sample_envelope();
        let mut changed = sample_envelope();
        if let Some(first) = changed.obligations.first_mut() {
            first.statement = "Changed statement".to_string();
        }

        let mut store = CapturedReceiptStoreV1::new();
        store
            .capture(receipt_set_for(&original))
            .map_err(|err| err.as_str().to_string())?;

        let original_report = evaluate_intent_plan_currentness(&store, &original)
            .map_err(|err| err.as_str().to_string())?;
        if original_report.status != BindingCurrentnessV1::Current {
            return Err(format!(
                "original intent plan must be current: {:?}",
                original_report.status
            ));
        }

        let changed_report = evaluate_intent_plan_currentness(&store, &changed)
            .map_err(|err| err.as_str().to_string())?;
        if changed_report.status != BindingCurrentnessV1::Missing {
            return Err(format!(
                "stale intent plan must be non-current (Missing), got {:?}",
                changed_report.status
            ));
        }
        if changed_report.plan_id == original_report.plan_id {
            return Err("changed intent plan must resolve to a distinct identity".into());
        }
        Ok(())
    }

    #[test]
    fn missing_receipts_report_missing_not_current() -> Result<(), String> {
        let envelope = sample_envelope();
        let store = CapturedReceiptStoreV1::new();
        let report = evaluate_intent_plan_currentness(&store, &envelope)
            .map_err(|err| err.as_str().to_string())?;
        if report.status != BindingCurrentnessV1::Missing {
            return Err(format!(
                "no captured receipts must be Missing, got {:?}",
                report.status
            ));
        }
        Ok(())
    }
}
