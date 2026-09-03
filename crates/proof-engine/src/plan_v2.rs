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
    let catalogs = canonical_catalogs(catalogs)?;
    let mut items = envelope
        .obligations
        .iter()
        .map(|obligation| build_item(obligation, &snapshot_identity, &catalogs))
        .collect::<Result<Vec<_>, _>>()?;
    let plan_id = semantic_plan_id(&intent_digest, &snapshot_identity, &catalogs, &items)?;

    if let Some(set) = receipts.get(&plan_id) {
        for binding in &set.bindings {
            match items.get_mut(binding.command_index) {
                Some(item) if item.disposition == ProofItemDispositionV1::SelectedForExecution => {
                    item.disposition = ProofItemDispositionV1::SatisfiedByCurrentReceipt;
                    item.execution_posture = ProofItemExecutionPostureV1::None;
                    item.selection = None;
                    item.current_receipt = Some(binding.receipt_digest.clone());
                    item.expected_receipt = None;
                }
                _ => {}
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
    let handoff = obligation
        .handoff
        .as_ref()
        .map(|handoff| {
            handoff.validate().map(|()| handoff).map_err(|error| {
                format!("obligation {} handoff: {error}", obligation.obligation_id)
            })
        })
        .transpose()?;
    let capability_class = handoff
        .and_then(|handoff| handoff.requested_evidence_class.clone())
        .unwrap_or_else(|| obligation.kind.as_str().to_string());
    let selected = if matches!(
        obligation.posture,
        intent_protocol::IntentObligationPostureV1::Decision
    ) || handoff.is_some_and(|handoff| {
        handoff.disposition
            != Some(intent_protocol::IntentProofHandoffDispositionV1::ReadyForProofPlanning)
    }) {
        None
    } else {
        catalogs
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
            )
    };

    let mut subject = ProofSubjectV1 {
        subject_class: ProofSubjectClassV1::Commit,
        revision: Some(snapshot_identity.to_string()),
        selector: Some(
            handoff
                .and_then(|handoff| handoff.subject_selector_ref.clone())
                .unwrap_or_else(|| obligation.evidence_refs.join(",")),
        ),
        body_identity: None,
        limitations: handoff
            .map(|handoff| handoff.subject_inventory_limitations.clone())
            .unwrap_or_default(),
    };
    if let Some(handoff) = handoff
        && handoff.subject_posture != Some(intent_protocol::IntentSubjectPostureV1::Exact)
    {
        subject
            .limitations
            .push("intent subject posture is not exact".to_string());
    }
    let base = format!("{}:{}", obligation.obligation_id, obligation.phase);
    let (disposition, selection, expected_receipt, posture, mut limitations) = match selected {
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
                    receipt_schema: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
                        .to_string(),
                    receipt_generation: 1,
                    config_identity: "config:unspecified".to_string(),
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
            match obligation.posture {
                intent_protocol::IntentObligationPostureV1::Decision => {
                    ProofItemDispositionV1::RepositoryDecisionRequired
                }
                _ => ProofItemDispositionV1::ProviderUnavailable,
            },
            None,
            None,
            ProofItemExecutionPostureV1::None,
            vec![match obligation.posture {
                intent_protocol::IntentObligationPostureV1::Decision => {
                    "repository decision required before provider selection".to_string()
                }
                _ => "no catalog capability matches the required evidence class".to_string(),
            }],
        ),
    };
    if let Some(handoff) = handoff
        && handoff.disposition
            != Some(intent_protocol::IntentProofHandoffDispositionV1::ReadyForProofPlanning)
    {
        let (disposition, reason) = match handoff.disposition {
            Some(intent_protocol::IntentProofHandoffDispositionV1::RepositoryDecisionRequired) => (
                ProofItemDispositionV1::RepositoryDecisionRequired,
                "intent handoff requires a repository decision",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::SelectorMissingOrAmbiguous) => (
                ProofItemDispositionV1::SelectorMissingOrAmbiguous,
                "intent handoff selector is missing or ambiguous",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::ManualOrNativeOutstanding) => (
                ProofItemDispositionV1::ManualOrNativeOutstanding,
                "intent handoff requires manual or native evidence",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::UnsupportedEvidenceClass) => (
                ProofItemDispositionV1::UnsupportedCapability,
                "intent handoff requests an unsupported evidence class",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::NotApplicableWithReason) => (
                ProofItemDispositionV1::NotApplicableWithReason,
                "intent handoff marks this obligation not applicable",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::PartialOrNotProven) => (
                ProofItemDispositionV1::NotProven,
                "intent handoff is partial or not proven",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::EvidenceDesignIncomplete)
            | None => (
                ProofItemDispositionV1::NotProven,
                "intent evidence design is incomplete",
            ),
            Some(intent_protocol::IntentProofHandoffDispositionV1::ReadyForProofPlanning) => (
                ProofItemDispositionV1::NotProven,
                "intent handoff readiness changed during planning",
            ),
        };
        limitations.push(
            handoff
                .disposition_reason
                .clone()
                .unwrap_or_else(|| reason.to_string()),
        );
        return Ok(ProofItemV1 {
            proof_item_id: digest(&base),
            intent_obligation_id: obligation.obligation_id.clone(),
            phase: obligation.phase.clone(),
            blocking: matches!(obligation.posture, intent_protocol::IntentObligationPostureV1::Blocking),
            evidence_purpose_ref: handoff
                .evidence_purpose_refs
                .first()
                .cloned()
                .unwrap_or_else(|| obligation.statement.clone()),
            required_capability_class: capability_class,
            snapshot_identity: snapshot_identity.to_string(),
            subject,
            disposition,
            selection: None,
            current_receipt: None,
            expected_receipt: None,
            execution_posture: ProofItemExecutionPostureV1::None,
            dependency_group: None,
            limitations,
            claim_boundary: "Planning disposition only; intent evidence handoff is not ready for provider execution.".to_string(),
        });
    }
    Ok(ProofItemV1 {
        proof_item_id: digest(&base),
        intent_obligation_id: obligation.obligation_id.clone(),
        phase: obligation.phase.clone(),
        blocking: matches!(obligation.posture, intent_protocol::IntentObligationPostureV1::Blocking),
        evidence_purpose_ref: handoff
            .and_then(|handoff| handoff.evidence_purpose_refs.first().cloned())
            .unwrap_or_else(|| obligation.statement.clone()),
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

fn canonical_catalogs(
    catalogs: &[ProofCapabilityCatalogV1],
) -> Result<Vec<ProofCapabilityCatalogV1>, String> {
    let mut canonical = catalogs.to_vec();
    for catalog in &mut canonical {
        validate_capability_catalog(catalog).map_err(|error| error.as_str().to_string())?;
        catalog
            .capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    }
    canonical.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
    Ok(canonical)
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
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationHandoffV1,
        IntentObligationPostureV1, IntentPhaseObligationKindV1, IntentPhaseObligationV1,
        IntentProofHandoffDispositionV1, IntentSubjectPostureV1, RepositorySnapshotV1,
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
                handoff: None,
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
        let first_item = first
            .items
            .first()
            .ok_or_else(|| "deterministic plan must contain an item".to_string())?;
        if first_item.disposition != ProofItemDispositionV1::SelectedForExecution {
            return Err("matching catalog capability must select execution".to_string());
        }
        Ok(())
    }

    #[test]
    fn catalog_permutations_have_one_semantic_identity() -> Result<(), String> {
        let mut reversed = catalog();
        reversed.provider_id = "provider-z".to_string();
        let mut first_catalog = catalog();
        first_catalog.provider_id = "provider-a".to_string();
        let receipts = CapturedReceiptStoreV1::new();
        let first = plan_proof_v2_from_intent(
            &envelope(),
            &[reversed.clone(), first_catalog.clone()],
            &receipts,
        )?;
        let second = plan_proof_v2_from_intent(&envelope(), &[first_catalog, reversed], &receipts)?;
        if first.plan_id != second.plan_id {
            return Err("catalog order must not change semantic plan identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn decision_posture_prevents_provider_selection() -> Result<(), String> {
        let mut decision = envelope();
        decision
            .obligations
            .get_mut(0)
            .ok_or_else(|| "fixture must contain an obligation".to_string())?
            .posture = IntentObligationPostureV1::Decision;
        let plan =
            plan_proof_v2_from_intent(&decision, &[catalog()], &CapturedReceiptStoreV1::new())?;
        let item = plan
            .items
            .into_iter()
            .next()
            .ok_or_else(|| "decision plan must contain an item".to_string())?;
        if item.disposition != ProofItemDispositionV1::RepositoryDecisionRequired
            || item.selection.is_some()
            || item.execution_posture != ProofItemExecutionPostureV1::None
            || item.limitations
                != vec!["repository decision required before provider selection".to_string()]
        {
            return Err("decision posture must remain non-executable".to_string());
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
    fn enriched_handoff_controls_proof_item_projection() -> Result<(), String> {
        let mut enriched = envelope();
        let obligation = enriched
            .obligations
            .first_mut()
            .ok_or_else(|| "missing obligation".to_string())?;
        obligation.handoff = Some(IntentObligationHandoffV1 {
            disposition: Some(IntentProofHandoffDispositionV1::ReadyForProofPlanning),
            evidence_purpose_refs: vec!["evidence:purpose".to_string()],
            requested_evidence_class: Some("cargo_allow_source_exception".to_string()),
            subject_selector_ref: Some("test:selector".to_string()),
            subject_posture: Some(IntentSubjectPostureV1::Exact),
            ..IntentObligationHandoffV1::default()
        });
        let plan = plan_proof_v2_from_intent(
            &enriched,
            &[ProofCapabilityCatalogV1::new(
                "provider-a",
                vec![ProofCapabilityV1 {
                    capability_id: "cargo_allow_source_exception".to_string(),
                    kind: ProofCapabilityKindV1::StaticReport,
                    program: "provider-a".to_string(),
                    statement: "source exception posture".to_string(),
                }],
            )],
            &CapturedReceiptStoreV1::new(),
        )?;
        let item = plan
            .items
            .first()
            .ok_or_else(|| "missing item".to_string())?;
        if item.required_capability_class != "cargo_allow_source_exception"
            || item.evidence_purpose_ref != "evidence:purpose"
            || item.subject.selector.as_deref() != Some("test:selector")
            || item.disposition != ProofItemDispositionV1::SelectedForExecution
        {
            return Err("enriched handoff was not preserved in the proof item".to_string());
        }
        Ok(())
    }

    #[test]
    fn non_ready_handoff_cannot_select_a_provider() -> Result<(), String> {
        let mut enriched = envelope();
        let obligation = enriched
            .obligations
            .first_mut()
            .ok_or_else(|| "missing obligation".to_string())?;
        obligation.handoff = Some(IntentObligationHandoffV1 {
            disposition: Some(IntentProofHandoffDispositionV1::PartialOrNotProven),
            subject_posture: Some(IntentSubjectPostureV1::Weak),
            requested_evidence_class: Some("evidence_review".to_string()),
            ..IntentObligationHandoffV1::default()
        });
        let plan =
            plan_proof_v2_from_intent(&enriched, &[catalog()], &CapturedReceiptStoreV1::new())?;
        let item = plan
            .items
            .first()
            .ok_or_else(|| "missing item".to_string())?;
        if item.disposition != ProofItemDispositionV1::NotProven
            || item.selection.is_some()
            || item.execution_posture != ProofItemExecutionPostureV1::None
        {
            return Err("non-ready handoff became executable".to_string());
        }
        Ok(())
    }

    #[test]
    fn not_applicable_handoff_preserves_reason() -> Result<(), String> {
        let mut enriched = envelope();
        let obligation = enriched
            .obligations
            .first_mut()
            .ok_or_else(|| "missing obligation".to_string())?;
        obligation.handoff = Some(IntentObligationHandoffV1 {
            disposition: Some(IntentProofHandoffDispositionV1::NotApplicableWithReason),
            disposition_reason: Some("provider cannot observe generated code".to_string()),
            ..IntentObligationHandoffV1::default()
        });
        let plan =
            plan_proof_v2_from_intent(&enriched, &[catalog()], &CapturedReceiptStoreV1::new())?;
        let item = plan
            .items
            .first()
            .ok_or_else(|| "missing item".to_string())?;
        if item.disposition != ProofItemDispositionV1::NotApplicableWithReason
            || !item
                .limitations
                .iter()
                .any(|reason| reason == "provider cannot observe generated code")
        {
            return Err("not-applicable reason was not preserved".to_string());
        }
        Ok(())
    }

    #[test]
    fn non_ready_handoff_dispositions_remain_explicit() -> Result<(), String> {
        let cases = [
            (
                IntentProofHandoffDispositionV1::RepositoryDecisionRequired,
                ProofItemDispositionV1::RepositoryDecisionRequired,
            ),
            (
                IntentProofHandoffDispositionV1::SelectorMissingOrAmbiguous,
                ProofItemDispositionV1::SelectorMissingOrAmbiguous,
            ),
            (
                IntentProofHandoffDispositionV1::ManualOrNativeOutstanding,
                ProofItemDispositionV1::ManualOrNativeOutstanding,
            ),
            (
                IntentProofHandoffDispositionV1::UnsupportedEvidenceClass,
                ProofItemDispositionV1::UnsupportedCapability,
            ),
            (
                IntentProofHandoffDispositionV1::NotApplicableWithReason,
                ProofItemDispositionV1::NotApplicableWithReason,
            ),
            (
                IntentProofHandoffDispositionV1::EvidenceDesignIncomplete,
                ProofItemDispositionV1::NotProven,
            ),
        ];
        for (disposition, expected) in cases {
            let mut enriched = envelope();
            let obligation = enriched
                .obligations
                .first_mut()
                .ok_or_else(|| "missing obligation".to_string())?;
            obligation.handoff = Some(IntentObligationHandoffV1 {
                disposition: Some(disposition),
                disposition_reason: (disposition
                    == IntentProofHandoffDispositionV1::NotApplicableWithReason)
                    .then(|| "not applicable to this change".to_string()),
                ..IntentObligationHandoffV1::default()
            });
            let plan =
                plan_proof_v2_from_intent(&enriched, &[catalog()], &CapturedReceiptStoreV1::new())?;
            let item = plan
                .items
                .first()
                .ok_or_else(|| "missing item".to_string())?;
            if item.disposition != expected
                || item.selection.is_some()
                || item.execution_posture != ProofItemExecutionPostureV1::None
            {
                return Err(format!("handoff disposition {disposition:?} was lowered"));
            }
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
        let reused_item = reused
            .items
            .first()
            .ok_or_else(|| "reused plan must contain an item".to_string())?;
        if reused_item.disposition != ProofItemDispositionV1::SatisfiedByCurrentReceipt {
            return Err("matching plan identity must permit receipt reuse".to_string());
        }
        Ok(())
    }
}
