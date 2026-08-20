//! Provider-neutral captured receipt status evaluation (#3600).

use proof_protocol::{
    CapturedReceiptManifestV1, ProofItemDispositionV1, ProofItemReceiptStatusV1, ProofPlanV2,
    validate_captured_receipt_manifest,
};
use serde::Serialize;

pub const RECEIPT_STATUS_REPORT_SCHEMA_ID: &str = "proof.receipt-status-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofItemReceiptStatusRowV1 {
    pub proof_item_id: String,
    pub status: ProofItemReceiptStatusV1,
    pub provider_id: Option<String>,
    pub capability_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReceiptStatusReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub items: Vec<ProofItemReceiptStatusRowV1>,
    pub claim_boundary: String,
}

pub fn evaluate_captured_receipt_status(
    plan: &ProofPlanV2,
    manifest: &CapturedReceiptManifestV1,
) -> Result<ReceiptStatusReportV1, String> {
    plan.validate()?;
    validate_captured_receipt_manifest(manifest)?;
    if manifest.plan_id != plan.plan_id {
        return Err(format!(
            "receipt manifest belongs to plan {}, expected {}",
            manifest.plan_id, plan.plan_id
        ));
    }

    let rows = plan
        .items
        .iter()
        .map(|item| {
            let Some(row) = manifest
                .rows
                .iter()
                .find(|row| row.proof_item_id == item.proof_item_id)
            else {
                return ProofItemReceiptStatusRowV1 {
                    proof_item_id: item.proof_item_id.clone(),
                    status: status_without_receipt(item.disposition),
                    provider_id: item
                        .selection
                        .as_ref()
                        .map(|selection| selection.provider_id.clone()),
                    capability_id: item
                        .selection
                        .as_ref()
                        .map(|selection| selection.capability_id.clone()),
                    reason: "no captured receipt row exists for this proof item".to_string(),
                };
            };

            if row.plan_id != plan.plan_id
                || row.snapshot_identity != plan.snapshot_identity
                || item
                    .selection
                    .as_ref()
                    .map(|selection| {
                        selection.provider_id != row.provider_id
                            || selection.capability_id != row.capability_id
                    })
                    .unwrap_or(true)
            {
                return ProofItemReceiptStatusRowV1 {
                    proof_item_id: item.proof_item_id.clone(),
                    status: ProofItemReceiptStatusV1::ReceiptForDifferentItem,
                    provider_id: Some(row.provider_id.clone()),
                    capability_id: Some(row.capability_id.clone()),
                    reason: "captured receipt identity does not match the selected proof item"
                        .to_string(),
                };
            }

            let (status, reason) = status_for_receipt(row);
            ProofItemReceiptStatusRowV1 {
                proof_item_id: item.proof_item_id.clone(),
                status,
                provider_id: Some(row.provider_id.clone()),
                capability_id: Some(row.capability_id.clone()),
                reason,
            }
        })
        .collect();

    Ok(ReceiptStatusReportV1 {
        schema_id: RECEIPT_STATUS_REPORT_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        items: rows,
        claim_boundary:
            "Captured provider evidence was validated and classified; no provider executed and no phase gate was opened."
                .to_string(),
    })
}

pub fn evaluate_captured_receipt_status_from_json(
    plan_json: &str,
    manifest_json: &str,
) -> Result<ReceiptStatusReportV1, String> {
    let plan: ProofPlanV2 =
        serde_json::from_str(plan_json).map_err(|error| format!("parse proof plan: {error}"))?;
    let manifest: CapturedReceiptManifestV1 = serde_json::from_str(manifest_json)
        .map_err(|error| format!("parse receipt manifest: {error}"))?;
    evaluate_captured_receipt_status(&plan, &manifest)
}

fn status_without_receipt(disposition: ProofItemDispositionV1) -> ProofItemReceiptStatusV1 {
    match disposition {
        ProofItemDispositionV1::ProviderUnavailable => {
            ProofItemReceiptStatusV1::ProviderUnavailable
        }
        ProofItemDispositionV1::ManualOrNativeOutstanding => {
            ProofItemReceiptStatusV1::ManualOrNativeOutstanding
        }
        ProofItemDispositionV1::NotApplicableWithReason => ProofItemReceiptStatusV1::NotApplicable,
        _ => ProofItemReceiptStatusV1::ReceiptMissing,
    }
}

fn status_for_receipt(
    row: &proof_protocol::CapturedReceiptManifestRowV1,
) -> (ProofItemReceiptStatusV1, String) {
    match row.receipt.currentness {
        effortless_repo_protocol::CurrentnessV1::Stale => {
            return (
                ProofItemReceiptStatusV1::ReceiptStale,
                "receipt currentness is stale".to_string(),
            );
        }
        effortless_repo_protocol::CurrentnessV1::NotProbed
        | effortless_repo_protocol::CurrentnessV1::PartialOrUnavailable => {
            return (
                ProofItemReceiptStatusV1::CurrentNotProven,
                "receipt currentness was not established completely".to_string(),
            );
        }
        effortless_repo_protocol::CurrentnessV1::Current => {}
    }
    if row.receipt.completeness != effortless_repo_protocol::CompletenessV1::Complete {
        return (
            ProofItemReceiptStatusV1::CurrentPartial,
            "receipt completeness is partial or unknown".to_string(),
        );
    }
    match row.receipt.result_class {
        effortless_repo_protocol::ResultClassV1::Completed => (
            ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt,
            "current complete receipt reports completion".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::Findings => (
            ProofItemReceiptStatusV1::CurrentFindings,
            "current complete receipt retains provider findings".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::PartialData => (
            ProofItemReceiptStatusV1::CurrentPartial,
            "provider reports partial data".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::Unsupported => (
            ProofItemReceiptStatusV1::CurrentUnsupported,
            "provider reports unsupported evidence".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::NotProven => (
            ProofItemReceiptStatusV1::CurrentNotProven,
            "provider does not establish the requested claim".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::InstrumentFailure => (
            ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            "provider reports an instrument failure".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::StaleInput => (
            ProofItemReceiptStatusV1::ReceiptStale,
            "provider reports stale input".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::Conflict => (
            ProofItemReceiptStatusV1::Conflict,
            "provider reports conflicting evidence".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::MalformedInput => (
            ProofItemReceiptStatusV1::ReceiptMalformed,
            "provider reports malformed input".to_string(),
        ),
        effortless_repo_protocol::ResultClassV1::Cancelled => (
            ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            "provider reports cancellation".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use effortless_repo_protocol::{
        AnalysisReceiptEnvelopeV1, ClaimBoundaryV1, RepositorySnapshotV1, ResolvedRevisionV1,
        ResultClassV1,
    };
    use proof_protocol::{
        ExpectedReceiptContractV1, ProofItemExecutionPostureV1, ProofItemV1, ProofSubjectClassV1,
        ProofSubjectV1, ProviderSelectionV1,
    };

    fn plan() -> ProofPlanV2 {
        ProofPlanV2::new(
            "plan-1",
            "intent-1",
            "snapshot-1",
            vec![ProofItemV1 {
                proof_item_id: "item-1".to_string(),
                intent_obligation_id: "obligation-1".to_string(),
                phase: "precommit".to_string(),
                blocking: true,
                evidence_purpose_ref: "purpose".to_string(),
                required_capability_class: "capability-1".to_string(),
                snapshot_identity: "snapshot-1".to_string(),
                subject: ProofSubjectV1 {
                    subject_class: ProofSubjectClassV1::Commit,
                    revision: Some("snapshot-1".to_string()),
                    selector: None,
                    body_identity: None,
                    limitations: Vec::new(),
                },
                disposition: ProofItemDispositionV1::SelectedForExecution,
                selection: Some(ProviderSelectionV1 {
                    provider_id: "provider-1".to_string(),
                    capability_id: "capability-1".to_string(),
                    request_digest: "request-1".to_string(),
                }),
                current_receipt: None,
                expected_receipt: Some(ExpectedReceiptContractV1 {
                    receipt_schema: "repo.analysis-receipt.v1".to_string(),
                    receipt_generation: 1,
                    currentness_dimensions: vec!["snapshot_identity".to_string()],
                }),
                execution_posture: ProofItemExecutionPostureV1::Execute,
                dependency_group: None,
                limitations: Vec::new(),
                claim_boundary: "test".to_string(),
            }],
        )
    }

    fn manifest(result_class: ResultClassV1) -> CapturedReceiptManifestV1 {
        let receipt = AnalysisReceiptEnvelopeV1::new(
            "provider-1",
            RepositorySnapshotV1::new_committed_head(
                "repo",
                "sha",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "commit".to_string(),
                    tree: "tree".to_string(),
                },
            ),
            result_class,
            "provider.payload.v1",
            serde_json::json!({"payload": true}),
            ClaimBoundaryV1::new("captured test evidence"),
        );
        CapturedReceiptManifestV1::new(
            "plan-1",
            vec![proof_protocol::CapturedReceiptManifestRowV1 {
                proof_item_id: "item-1".to_string(),
                plan_id: "plan-1".to_string(),
                provider_id: "provider-1".to_string(),
                capability_id: "capability-1".to_string(),
                snapshot_identity: "snapshot-1".to_string(),
                receipt,
            }],
        )
    }

    #[test]
    fn findings_remain_findings() -> Result<(), String> {
        let report = evaluate_captured_receipt_status(&plan(), &manifest(ResultClassV1::Findings))?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::CurrentFindings)
        {
            return Err("provider findings were flattened".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_receipt_is_not_success() -> Result<(), String> {
        let empty = CapturedReceiptManifestV1::new("plan-1", Vec::new());
        let report = evaluate_captured_receipt_status(&plan(), &empty)?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::ReceiptMissing)
        {
            return Err("missing receipt became success".to_string());
        }
        Ok(())
    }
}
