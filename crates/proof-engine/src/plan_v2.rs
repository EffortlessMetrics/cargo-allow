//! Deterministic evidence-oriented plan generation (#3599).

use crate::captured_receipts::{CapturedReceiptStoreV1, validate_captured_receipt_store};
use crate::intent_digest::intent_obligation_plan_digest;
use intent_protocol::{IntentObligationPlanEnvelopeV1, IntentPhaseObligationV1};
use proof_protocol::{
    ExpectedReceiptContractV1, ProofCapabilityCatalogV1, ProofItemDispositionV1,
    ProofItemExecutionPostureV1, ProofItemV1, ProofPlanV2, ProofSubjectClassV1, ProofSubjectV1,
    ProviderSelectionV1, validate_capability_catalog,
};
use sha2::{Digest, Sha256};

pub const PLAN_V2_PLANNER_SCHEMA_ID: &str = "proof.plan-v2-planner.v1";

/// Generate the complete semantic plan from explicit, deterministic inputs.
/// No provider is registered or executed here; catalogs only advertise what
/// a later provider stage may select.
pub fn plan_proof_v2_from_intent(
    envelope: &IntentObligationPlanEnvelopeV1,
    catalogs: &[ProofCapabilityCatalogV1],
    receipts: &CapturedReceiptStoreV1,
) -> Result<ProofPlanV2, String> {
    let intent_digest = intent_obligation_plan_digest(envelope)?;
    for catalog in catalogs {
        validate_capability_catalog(catalog).map_err(|error| error.as_str().to_string())?;
    }
    validate_captured_receipt_store(receipts).map_err(|error| error.as_str().to_string())?;

    let snapshot_identity = snapshot_identity(envelope)?;
    let mut items = envelope
        .obligations
        .iter()
        .map(|obligation| build_item(obligation, &snapshot_identity, catalogs))
        .collect::<Result<Vec<_>, _>>()?;
    let plan_id = semantic_plan_id(&intent_digest, &snapshot_identity, catalogs, &items)?;

    if let Some(set) = receipts.get(&plan_id) {
        for binding in &set.bindings {
            if let Some(item) = items.get_mut(binding.command_index) {
                if item.disposition == ProofItemDispositionV1::SelectedForExecution {
                    item.disposition = ProofItemDispositionV1::SatisfiedByCurrentReceipt;
                    item.execution_posture = ProofItemExecutionPostureV1::None;
                    item.selection = None;
                    item.current_receipt = Some(binding.receipt_digest.clone());
                    item.expected_receipt = None;
                }
            }
        }
    }

    let plan = ProofPlanV2::new(plan_id, intent_digest, snapshot_identity, items);
    plan.validate()?;
    Ok(plan)
}

fn build_item(
    obligation: &IntentPhaseObligationV1,
    snapshot_identity: &str,
    catalogs: &[ProofCapabilityCatalogV1],
) -> Result<ProofItemV1, String> {
    let capability_class = obligation.kind.as_str().to_string();
    let selected = catalogs
        .iter()
        .flat_map(|catalog| {
            catalog
                .capabilities
                .iter()
                .map(move |capability| (catalog, capability))
        })
        .filter(|(_, capability)| capability.capability_id == capability_class)
        .min_by(
            |(left_catalog, left_capability), (right_catalog, right_capability)| {
                (
                    left_catalog.provider_id.as_str(),
                    left_capability.capability_id.as_str(),
                )
                    .cmp(&(
                        right_catalog.provider_id.as_str(),
                        right_capability.capability_id.as_str(),
                    ))
            },
        );

    let subject = ProofSubjectV1 {
        subject_class: ProofSubjectClassV1::Commit,
        revision: Some(snapshot_identity.to_string()),
        selector: Some(obligation.evidence_refs.join(",")),
        body_identity: None,
        limitations: Vec::new(),
    };
    let base = format!("{}:{}", obligation.obligation_id, obligation.phase);
    let (disposition, selection, expected_receipt, posture, limitations) = match selected {
        Some((catalog, capability)) => {
            let request_digest = digest(&format!(
                "{}:{}:{}",
                catalog.provider_id, capability.capability_id, base
            ));
            (
                ProofItemDispositionV1::SelectedForExecution,
                Some(ProviderSelectionV1 {
                    provider_id: catalog.provider_id.clone(),
                    capability_id: capability.capability_id.clone(),
                    request_digest,
                }),
                Some(ExpectedReceiptContractV1 {
                    receipt_schema: "proof.receipt-set.v1".to_string(),
                    receipt_generation: 1,
                    currentness_dimensions: vec![
                        "snapshot_identity".to_string(),
                        "subject".to_string(),
                        "provider_request".to_string(),
                        "config".to_string(),
                    ],
                }),
                ProofItemExecutionPostureV1::Execute,
                Vec::new(),
            )
        }
        None => (
            ProofItemDispositionV1::ProviderUnavailable,
            None,
            None,
            ProofItemExecutionPostureV1::None,
            vec!["no catalog capability matches the required evidence class".to_string()],
        ),
    };
    Ok(ProofItemV1 {
        proof_item_id: digest(&base),
        intent_obligation_id: obligation.obligation_id.clone(),
        phase: obligation.phase.clone(),
        blocking: matches!(obligation.posture, intent_protocol::IntentObligationPostureV1::Blocking),
        evidence_purpose_ref: obligation.statement.clone(),
        required_capability_class: capability_class,
        snapshot_identity: snapshot_identity.to_string(),
        subject,
        disposition,
        selection,
        current_receipt: None,
        expected_receipt,
        execution_posture: posture,
        dependency_group: None,
        limitations,
        claim_boundary: "Planning disposition only; no provider execution or evidence satisfaction is established.".to_string(),
    })
}

fn snapshot_identity(envelope: &IntentObligationPlanEnvelopeV1) -> Result<String, String> {
    let canonical = serde_json::to_string(&envelope.identity.snapshot)
        .map_err(|error| format!("serialize repository snapshot: {error}"))?;
    Ok(digest(&canonical))
}

fn semantic_plan_id(
    intent_digest: &str,
    snapshot_identity: &str,
    catalogs: &[ProofCapabilityCatalogV1],
    items: &[ProofItemV1],
) -> Result<String, String> {
    let canonical = serde_json::to_string(&(intent_digest, snapshot_identity, catalogs, items))
        .map_err(|error| format!("serialize semantic plan inputs: {error}"))?;
    Ok(format!("proof-plan-v2:{}", digest(&canonical)))
}

fn digest(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!(
        "sha256:v1:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPostureV1,
        IntentPhaseObligationKindV1, IntentPhaseObligationV1, RepositorySnapshotV1,
        ResolvedRevisionV1,
    };
    use proof_protocol::{ProofCapabilityKindV1, ProofCapabilityV1, ProofReceiptSetV1};

    fn envelope() -> IntentObligationPlanEnvelopeV1 {
        IntentObligationPlanEnvelopeV1::new(
            IntentIdentityEnvelopeV1::new(
                RepositorySnapshotV1::new_committed_head(
                    "repo",
                    "sha1",
                    ResolvedRevisionV1 {
                        requested: "HEAD".to_string(),
                        commit: "commit-1".to_string(),
                        tree: "tree-1".to_string(),
                    },
                ),
                IntentArtifactKindV1::RequirementDocument,
                "req-1",
                "requirements.md",
                "body-1",
            ),
            "precommit",
            vec![IntentPhaseObligationV1 {
                obligation_id: "obl-1".to_string(),
                phase: "precommit".to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "review evidence".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec!["doc:README.md".to_string()],
            }],
        )
    }

    fn catalog() -> ProofCapabilityCatalogV1 {
        ProofCapabilityCatalogV1::new(
            "provider-a",
            vec![ProofCapabilityV1 {
                capability_id: "evidence_review".to_string(),
                kind: ProofCapabilityKindV1::StaticReport,
                program: "provider-a".to_string(),
                statement: "evidence review".to_string(),
            }],
        )
    }

    #[test]
    fn planner_is_deterministic_and_binds_catalog() -> Result<(), String> {
        let receipts = CapturedReceiptStoreV1::new();
        let first = plan_proof_v2_from_intent(&envelope(), &[catalog()], &receipts)?;
        let second = plan_proof_v2_from_intent(&envelope(), &[catalog()], &receipts)?;
        if first != second {
            return Err("identical inputs must produce identical plans".to_string());
        }
        if first.items[0].disposition != ProofItemDispositionV1::SelectedForExecution {
            return Err("matching catalog capability must select execution".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_capability_is_explicitly_unavailable() -> Result<(), String> {
        let plan = plan_proof_v2_from_intent(&envelope(), &[], &CapturedReceiptStoreV1::new())?;
        let item = plan
            .items
            .first()
            .ok_or_else(|| "missing item".to_string())?;
        if item.disposition != ProofItemDispositionV1::ProviderUnavailable
            || item.execution_posture != ProofItemExecutionPostureV1::None
        {
            return Err("missing catalog capability must fail closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn receipt_reuse_requires_exact_plan_identity() -> Result<(), String> {
        let initial =
            plan_proof_v2_from_intent(&envelope(), &[catalog()], &CapturedReceiptStoreV1::new())?;
        let binding = proof_protocol::ProofReceiptBindingV1 {
            binding_id: "binding-1".to_string(),
            plan_id: initial.plan_id.clone(),
            command_index: 0,
            analysis_receipt_schema_id: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
                .to_string(),
            receipt_digest: "receipt-1".to_string(),
        };
        let mut receipts = CapturedReceiptStoreV1::new();
        receipts
            .capture(ProofReceiptSetV1::new(initial.plan_id, vec![binding]))
            .map_err(|error| error.as_str().to_string())?;
        let reused = plan_proof_v2_from_intent(&envelope(), &[catalog()], &receipts)?;
        if reused.items[0].disposition != ProofItemDispositionV1::SatisfiedByCurrentReceipt {
            return Err("matching plan identity must permit receipt reuse".to_string());
        }
        Ok(())
    }
}
