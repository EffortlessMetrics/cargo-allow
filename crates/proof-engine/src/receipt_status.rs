//! Provider-neutral captured receipt status evaluation (#3600).

use effortless_repo_protocol::stable_digest_json;
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
    for row in &manifest.rows {
        if !plan
            .items
            .iter()
            .any(|item| item.proof_item_id == row.proof_item_id)
        {
            return Err(format!(
                "receipt row {} does not belong to proof plan {}",
                row.proof_item_id, plan.plan_id
            ));
        }
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

            if let Ok(embedded_snapshot_identity) = stable_digest_json(&row.receipt.snapshot)
                && row.snapshot_identity != embedded_snapshot_identity
            {
                return ProofItemReceiptStatusRowV1 {
                    proof_item_id: item.proof_item_id.clone(),
                    status: ProofItemReceiptStatusV1::ReceiptForDifferentItem,
                    provider_id: Some(row.provider_id.clone()),
                    capability_id: Some(row.capability_id.clone()),
                    reason: "embedded receipt snapshot identity does not match the manifest"
                        .to_string(),
                };
            }

            if let Some((status, reason)) = state_machine_binding_mismatch(item, row) {
                return ProofItemReceiptStatusRowV1 {
                    proof_item_id: item.proof_item_id.clone(),
                    status,
                    provider_id: Some(row.provider_id.clone()),
                    capability_id: Some(row.capability_id.clone()),
                    reason,
                };
            }

            if let Some(reason) = receipt_contract_mismatch(item, row) {
                return ProofItemReceiptStatusRowV1 {
                    proof_item_id: item.proof_item_id.clone(),
                    status: ProofItemReceiptStatusV1::ReceiptMalformed,
                    provider_id: Some(row.provider_id.clone()),
                    capability_id: Some(row.capability_id.clone()),
                    reason,
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

fn state_machine_binding_mismatch(
    item: &proof_protocol::ProofItemV1,
    row: &proof_protocol::CapturedReceiptManifestRowV1,
) -> Option<(ProofItemReceiptStatusV1, String)> {
    let expected = item
        .expected_receipt
        .as_ref()
        .and_then(|contract| contract.state_machine.as_ref());
    let observed = row.state_machine.as_ref();
    match (expected, observed) {
        (None, None) => None,
        (Some(_), None) => Some((
            ProofItemReceiptStatusV1::ReceiptMalformed,
            "expected state-machine evidence binding is missing from the receipt".to_string(),
        )),
        (None, Some(_)) => Some((
            ProofItemReceiptStatusV1::ReceiptMalformed,
            "receipt carries state-machine evidence not requested by this plan item".to_string(),
        )),
        (Some(expected), Some(observed))
            if expected.machine_id != observed.machine_id
                || expected.property_id != observed.property_id
                || expected.evidence_kind != observed.evidence_kind =>
        {
            Some((
                ProofItemReceiptStatusV1::ReceiptForDifferentItem,
                "captured state-machine machine, property or evidence kind differs from the requested claim"
                    .to_string(),
            ))
        }
        (Some(expected), Some(observed)) if expected != observed => Some((
            ProofItemReceiptStatusV1::ReceiptStale,
            "captured state-machine model, property, assumptions, scope, toolchain, implementation or adapter identity has changed"
                .to_string(),
        )),
        (Some(_), Some(_)) => None,
    }
}

fn receipt_contract_mismatch(
    item: &proof_protocol::ProofItemV1,
    row: &proof_protocol::CapturedReceiptManifestRowV1,
) -> Option<String> {
    let expected = item.expected_receipt.as_ref()?;
    let embedded_snapshot_identity = match stable_digest_json(&row.receipt.snapshot) {
        Ok(identity) => identity,
        Err(error) => return Some(format!("embedded snapshot identity failed: {error:?}")),
    };
    if row.snapshot_identity != embedded_snapshot_identity {
        return Some("embedded snapshot identity does not match manifest identity".to_string());
    }
    if expected.receipt_schema != row.receipt.schema_id {
        return Some(format!(
            "receipt schema {} does not match expected {}",
            row.receipt.schema_id, expected.receipt_schema
        ));
    }
    if expected.receipt_generation != row.receipt_generation {
        return Some(format!(
            "receipt generation {} does not match expected {}",
            row.receipt_generation, expected.receipt_generation
        ));
    }
    for dimension in &expected.currentness_dimensions {
        let present = match dimension.as_str() {
            "snapshot" | "snapshot_identity" => true,
            "subject" => item
                .subject
                .body_identity
                .as_ref()
                .or(item.subject.selector.as_ref())
                .or(item.subject.revision.as_ref())
                .map(|identity| identity == &row.subject_identity)
                .unwrap_or(false),
            "provider_request" => item
                .selection
                .as_ref()
                .map(|selection| selection.request_digest == row.provider_request_identity)
                .unwrap_or(false),
            "config" => row.config_identity == expected.config_identity,
            _ => false,
        };
        if !present {
            return Some(format!(
                "expected receipt currentness dimension {dimension} is not bound"
            ));
        }
    }
    None
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
        let snapshot_identity = snapshot_identity();
        ProofPlanV2::new(
            "plan-1",
            "intent-1",
            snapshot_identity.clone(),
            vec![ProofItemV1 {
                proof_item_id: "item-1".to_string(),
                intent_obligation_id: "obligation-1".to_string(),
                phase: "precommit".to_string(),
                blocking: true,
                evidence_purpose_ref: "purpose".to_string(),
                required_capability_class: "capability-1".to_string(),
                snapshot_identity: snapshot_identity.clone(),
                subject: ProofSubjectV1 {
                    subject_class: ProofSubjectClassV1::Commit,
                    revision: Some(snapshot_identity.clone()),
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
                    state_machine: None,
                    receipt_schema: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
                        .to_string(),
                    receipt_generation: 1,
                    config_identity: "config:test".to_string(),
                    currentness_dimensions: vec![
                        "snapshot_identity".to_string(),
                        "subject".to_string(),
                        "provider_request".to_string(),
                        "config".to_string(),
                    ],
                }),
                execution_posture: ProofItemExecutionPostureV1::Execute,
                dependency_group: None,
                limitations: Vec::new(),
                claim_boundary: "test".to_string(),
            }],
        )
    }

    fn manifest(result_class: ResultClassV1) -> CapturedReceiptManifestV1 {
        let embedded_snapshot = snapshot();
        let receipt = AnalysisReceiptEnvelopeV1::new(
            "provider-1",
            embedded_snapshot,
            result_class,
            "provider.payload.v1",
            serde_json::json!({"payload": true}),
            ClaimBoundaryV1::new("captured test evidence"),
        );
        CapturedReceiptManifestV1::new(
            "plan-1",
            vec![proof_protocol::CapturedReceiptManifestRowV1 {
                state_machine: None,
                proof_item_id: "item-1".to_string(),
                plan_id: "plan-1".to_string(),
                provider_id: "provider-1".to_string(),
                capability_id: "capability-1".to_string(),
                snapshot_identity: snapshot_identity(),
                subject_identity: snapshot_identity(),
                provider_request_identity: "request-1".to_string(),
                config_identity: "config:test".to_string(),
                receipt_generation: 1,
                receipt,
            }],
        )
    }

    fn snapshot() -> RepositorySnapshotV1 {
        RepositorySnapshotV1::new_committed_head(
            "snapshot-1",
            "sha",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "commit".to_string(),
                tree: "tree".to_string(),
            },
        )
    }

    fn snapshot_identity() -> String {
        stable_digest_json(&snapshot()).unwrap_or_else(|_| "invalid-snapshot".to_string())
    }

    fn state_machine_fixture() -> Result<(ProofPlanV2, CapturedReceiptManifestV1), String> {
        use effortless_repo_protocol::stable_digest_hex;
        let binding = proof_protocol::StateMachineEvidenceBindingV1 {
            schema_id: proof_protocol::STATE_MACHINE_EVIDENCE_BINDING_SCHEMA_ID.to_string(),
            machine_id: "fixture.repair-attempt".to_string(),
            property_id: "finish-requires-current-snapshot".to_string(),
            evidence_kind: proof_protocol::StateMachineEvidenceKindV1::ImplementationConformance,
            model_digest: stable_digest_hex(b"synthetic model"),
            property_digest: stable_digest_hex(b"synthetic property"),
            assumptions_digest: stable_digest_hex(b"synthetic assumptions"),
            scope_digest: stable_digest_hex(b"synthetic scope: two states, one step"),
            toolchain_digest: stable_digest_hex(b"synthetic toolchain; no native tool ran"),
            implementation_digest: Some(stable_digest_hex(b"synthetic implementation")),
            adapter_digest: Some(stable_digest_hex(b"synthetic driver")),
        };
        let mut plan = plan();
        plan.items
            .first_mut()
            .and_then(|item| item.expected_receipt.as_mut())
            .ok_or_else(|| "fixture expected receipt missing".to_string())?
            .state_machine = Some(binding.clone());
        let mut manifest = manifest(ResultClassV1::Completed);
        manifest
            .rows
            .first_mut()
            .ok_or_else(|| "fixture captured row missing".to_string())?
            .state_machine = Some(binding);
        Ok((plan, manifest))
    }

    fn state_machine_status(
        plan: &ProofPlanV2,
        manifest: &CapturedReceiptManifestV1,
    ) -> Result<ProofItemReceiptStatusV1, String> {
        evaluate_captured_receipt_status(plan, manifest)?
            .items
            .first()
            .map(|row| row.status)
            .ok_or_else(|| "state-machine fixture produced no result".to_string())
    }

    #[test]
    fn state_machine_matching_binding_preserves_result_class() -> Result<(), String> {
        let (plan, mut manifest) = state_machine_fixture()?;
        if state_machine_status(&plan, &manifest)?
            != ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt
        {
            return Err("matching binding did not reach captured result evaluation".to_string());
        }
        for (result, expected) in [
            (
                ResultClassV1::Findings,
                ProofItemReceiptStatusV1::CurrentFindings,
            ),
            (
                ResultClassV1::PartialData,
                ProofItemReceiptStatusV1::CurrentPartial,
            ),
            (
                ResultClassV1::Unsupported,
                ProofItemReceiptStatusV1::CurrentUnsupported,
            ),
            (
                ResultClassV1::NotProven,
                ProofItemReceiptStatusV1::CurrentNotProven,
            ),
            (
                ResultClassV1::InstrumentFailure,
                ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            ),
        ] {
            manifest
                .rows
                .first_mut()
                .ok_or_else(|| "fixture row missing".to_string())?
                .receipt
                .result_class = result;
            if state_machine_status(&plan, &manifest)? != expected {
                return Err(format!("matching binding promoted {result:?} to success"));
            }
        }
        Ok(())
    }

    #[test]
    fn state_machine_each_input_change_invalidates_receipt() -> Result<(), String> {
        let (original_plan, manifest) = state_machine_fixture()?;
        for field in [
            "model_digest",
            "property_digest",
            "assumptions_digest",
            "scope_digest",
            "toolchain_digest",
            "implementation_digest",
            "adapter_digest",
        ] {
            let mut value =
                serde_json::to_value(&original_plan).map_err(|error| error.to_string())?;
            let binding = value
                .get_mut("items")
                .and_then(serde_json::Value::as_array_mut)
                .and_then(|items| items.first_mut())
                .and_then(|item| item.get_mut("expected_receipt"))
                .and_then(|expected| expected.get_mut("state_machine"))
                .ok_or_else(|| "fixture binding missing".to_string())?;
            *binding
                .get_mut(field)
                .ok_or_else(|| format!("fixture {field} missing"))? = serde_json::Value::String(
                effortless_repo_protocol::stable_digest_hex(b"changed input"),
            );
            let changed_plan: ProofPlanV2 =
                serde_json::from_value(value).map_err(|error| error.to_string())?;
            if state_machine_status(&changed_plan, &manifest)?
                != ProofItemReceiptStatusV1::ReceiptStale
            {
                return Err(format!("changed {field} with unchanged snapshot reused old evidence"));
            }
        }
        Ok(())
    }

    #[test]
    fn state_machine_wrong_subject_and_kind_are_rejected() -> Result<(), String> {
        let (plan, original_manifest) = state_machine_fixture()?;
        for changed in ["machine", "property", "kind", "model_only"] {
            let mut manifest = original_manifest.clone();
            let binding = manifest
                .rows
                .first_mut()
                .and_then(|row| row.state_machine.as_mut())
                .ok_or_else(|| "fixture binding missing".to_string())?;
            match changed {
                "machine" => binding.machine_id = "another-machine".to_string(),
                "property" => binding.property_id = "another-property".to_string(),
                "kind" => {
                    binding.evidence_kind =
                        proof_protocol::StateMachineEvidenceKindV1::ImplementationProof;
                }
                _ => {
                    binding.evidence_kind = proof_protocol::StateMachineEvidenceKindV1::ModelCheck;
                    binding.implementation_digest = None;
                    binding.adapter_digest = None;
                }
            }
            if state_machine_status(&plan, &manifest)?
                != ProofItemReceiptStatusV1::ReceiptForDifferentItem
            {
                return Err(format!("wrong state-machine {changed} satisfied the requested claim"));
            }
        }
        Ok(())
    }

    #[test]
    fn state_machine_binding_cannot_be_missing_or_unsolicited() -> Result<(), String> {
        let (original_plan, original_manifest) = state_machine_fixture()?;
        for missing in ["expected", "observed"] {
            let mut plan = original_plan.clone();
            let mut manifest = original_manifest.clone();
            if missing == "expected" {
                plan.items
                    .first_mut()
                    .and_then(|item| item.expected_receipt.as_mut())
                    .ok_or_else(|| "fixture contract missing".to_string())?
                    .state_machine = None;
            } else {
                manifest
                    .rows
                    .first_mut()
                    .ok_or_else(|| "fixture row missing".to_string())?
                    .state_machine = None;
            }
            if state_machine_status(&plan, &manifest)?
                != ProofItemReceiptStatusV1::ReceiptMalformed
            {
                return Err(format!("missing {missing} state-machine identity was accepted"));
            }
        }
        Ok(())
    }

    #[test]
    fn state_machine_binding_does_not_replace_base_currentness_dimensions() -> Result<(), String> {
        let (original_plan, manifest) = state_machine_fixture()?;
        for dimension in ["snapshot_identity", "subject", "provider_request", "config"] {
            let mut plan = original_plan.clone();
            plan.items
                .first_mut()
                .and_then(|item| item.expected_receipt.as_mut())
                .ok_or_else(|| "fixture contract missing".to_string())?
                .currentness_dimensions
                .retain(|declared| declared != dimension);
            match state_machine_status(&plan, &manifest) {
                Err(reason) if reason.contains("state-machine binding requires") => {}
                other => return Err(format!("missing {dimension} produced {other:?}")),
            }
        }
        Ok(())
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
    fn subject_currentness_binds_body_identity_before_selector() -> Result<(), String> {
        let mut bound_plan = plan();
        let item = bound_plan
            .items
            .first_mut()
            .ok_or_else(|| "plan item missing".to_string())?;
        item.subject.selector = Some("selector:old".to_string());
        item.subject.body_identity = Some("source:staged:new".to_string());

        let mut matching_body = manifest(ResultClassV1::Completed);
        matching_body
            .rows
            .first_mut()
            .ok_or_else(|| "manifest row missing".to_string())?
            .subject_identity = "source:staged:new".to_string();
        let report = evaluate_captured_receipt_status(&bound_plan, &matching_body)?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt)
        {
            return Err("matching source identity was not accepted".to_string());
        }

        let mut selector_only = manifest(ResultClassV1::Completed);
        selector_only
            .rows
            .first_mut()
            .ok_or_else(|| "manifest row missing".to_string())?
            .subject_identity = "selector:old".to_string();
        let report = evaluate_captured_receipt_status(&bound_plan, &selector_only)?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::ReceiptMalformed)
        {
            return Err("selector-only receipt bypassed source binding".to_string());
        }
        Ok(())
    }

    #[test]
    fn every_provider_result_class_has_an_explicit_status() -> Result<(), String> {
        let expected = [
            (
                ResultClassV1::Completed,
                ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt,
            ),
            (
                ResultClassV1::Findings,
                ProofItemReceiptStatusV1::CurrentFindings,
            ),
            (
                ResultClassV1::PartialData,
                ProofItemReceiptStatusV1::CurrentPartial,
            ),
            (
                ResultClassV1::Unsupported,
                ProofItemReceiptStatusV1::CurrentUnsupported,
            ),
            (
                ResultClassV1::NotProven,
                ProofItemReceiptStatusV1::CurrentNotProven,
            ),
            (
                ResultClassV1::InstrumentFailure,
                ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            ),
            (
                ResultClassV1::StaleInput,
                ProofItemReceiptStatusV1::ReceiptStale,
            ),
            (ResultClassV1::Conflict, ProofItemReceiptStatusV1::Conflict),
            (
                ResultClassV1::MalformedInput,
                ProofItemReceiptStatusV1::ReceiptMalformed,
            ),
            (
                ResultClassV1::Cancelled,
                ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            ),
        ];
        for (result_class, expected_status) in expected {
            let report = evaluate_captured_receipt_status(&plan(), &manifest(result_class))?;
            let actual = report
                .items
                .first()
                .map(|item| item.status)
                .ok_or_else(|| "result class produced no status row".to_string())?;
            if actual != expected_status {
                return Err(format!(
                    "{result_class:?} mapped to {actual:?}, expected {expected_status:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn stale_partial_and_unprobed_receipts_are_not_current() -> Result<(), String> {
        let mut stale = manifest(ResultClassV1::Completed);
        stale
            .rows
            .first_mut()
            .ok_or_else(|| "stale fixture row missing".to_string())?
            .receipt
            .currentness = effortless_repo_protocol::CurrentnessV1::Stale;
        if evaluate_captured_receipt_status(&plan(), &stale)?
            .items
            .first()
            .map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::ReceiptStale)
        {
            return Err("stale receipt was not classified as stale".to_string());
        }

        let mut partial = manifest(ResultClassV1::Completed);
        partial
            .rows
            .first_mut()
            .ok_or_else(|| "partial fixture row missing".to_string())?
            .receipt
            .completeness = effortless_repo_protocol::CompletenessV1::Partial;
        if evaluate_captured_receipt_status(&plan(), &partial)?
            .items
            .first()
            .map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::CurrentPartial)
        {
            return Err("partial receipt was not classified as partial".to_string());
        }

        let mut unprobed = manifest(ResultClassV1::Completed);
        unprobed
            .rows
            .first_mut()
            .ok_or_else(|| "unprobed fixture row missing".to_string())?
            .receipt
            .currentness = effortless_repo_protocol::CurrentnessV1::NotProbed;
        if evaluate_captured_receipt_status(&plan(), &unprobed)?
            .items
            .first()
            .map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::CurrentNotProven)
        {
            return Err("unprobed receipt was not classified as not proven".to_string());
        }
        Ok(())
    }

    #[test]
    fn currentness_dimensions_are_each_enforced() -> Result<(), String> {
        for dimension in ["subject", "provider_request", "config", "unknown"] {
            let mut candidate_plan = plan();
            let mut candidate_manifest = manifest(ResultClassV1::Completed);
            match dimension {
                "subject" => {
                    candidate_manifest
                        .rows
                        .first_mut()
                        .ok_or_else(|| "subject fixture row missing".to_string())?
                        .subject_identity = "wrong".to_string()
                }
                "provider_request" => {
                    candidate_manifest
                        .rows
                        .first_mut()
                        .ok_or_else(|| "request fixture row missing".to_string())?
                        .provider_request_identity = "wrong".to_string()
                }
                "config" => {
                    candidate_manifest
                        .rows
                        .first_mut()
                        .ok_or_else(|| "config fixture row missing".to_string())?
                        .config_identity = "wrong".to_string()
                }
                "unknown" => candidate_plan
                    .items
                    .first_mut()
                    .ok_or_else(|| "fixture item missing".to_string())?
                    .expected_receipt
                    .as_mut()
                    .ok_or_else(|| "expected receipt missing".to_string())?
                    .currentness_dimensions
                    .push("unknown".to_string()),
                _ => {}
            }
            let report = evaluate_captured_receipt_status(&candidate_plan, &candidate_manifest)?;
            if report.items.first().map(|item| item.status)
                != Some(ProofItemReceiptStatusV1::ReceiptMalformed)
            {
                return Err(format!("{dimension} currentness mismatch was accepted"));
            }
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

    #[test]
    fn unknown_manifest_row_is_rejected() -> Result<(), String> {
        let mut manifest = manifest(ResultClassV1::Completed);
        let mut extra = manifest
            .rows
            .first()
            .cloned()
            .ok_or_else(|| "fixture row missing".to_string())?;
        extra.proof_item_id = "unknown-item".to_string();
        manifest.rows.push(extra);
        match evaluate_captured_receipt_status(&plan(), &manifest) {
            Err(message) if message.contains("does not belong") => Ok(()),
            Err(message) => Err(format!("unexpected rejection: {message}")),
            Ok(_) => Err("unknown manifest row was accepted".to_string()),
        }
    }

    #[test]
    fn embedded_snapshot_mismatch_is_not_current() -> Result<(), String> {
        let mut manifest = manifest(ResultClassV1::Completed);
        let row = manifest
            .rows
            .first_mut()
            .ok_or_else(|| "fixture row missing".to_string())?;
        row.receipt.snapshot.root_identity = "other-snapshot".to_string();
        let report = evaluate_captured_receipt_status(&plan(), &manifest)?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::ReceiptForDifferentItem)
        {
            return Err("embedded snapshot mismatch was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn expected_schema_and_generation_mismatch_are_malformed() -> Result<(), String> {
        for mutate in [0_u8, 1_u8] {
            let mut manifest = manifest(ResultClassV1::Completed);
            let mut plan = plan();
            if mutate == 0 {
                let item = plan
                    .items
                    .first_mut()
                    .ok_or_else(|| "fixture item missing".to_string())?;
                if let Some(expected) = item.expected_receipt.as_mut() {
                    expected.receipt_schema = "wrong.schema.v1".to_string();
                }
            } else {
                let row = manifest
                    .rows
                    .first_mut()
                    .ok_or_else(|| "fixture row missing".to_string())?;
                row.receipt_generation = 2;
            }
            let report = evaluate_captured_receipt_status(&plan, &manifest)?;
            if report.items.first().map(|item| item.status)
                != Some(ProofItemReceiptStatusV1::ReceiptMalformed)
            {
                return Err("receipt contract mismatch was accepted".to_string());
            }
        }
        Ok(())
    }
}
