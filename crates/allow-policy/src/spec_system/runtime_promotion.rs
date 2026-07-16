use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    EvidenceDispositionState, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, RequirementDelta, RequirementGraph,
    RequirementId, RequirementStatus, SupportClaimDispositionState,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePromotionFindingCode {
    RequirementNotFound,
    RequirementGenerationMismatch,
    RequirementStatusMismatch,
    RequirementStatusDoesNotAllowImplementation,
    SpecOnlyRuntimeImplementationClaim,
    RuntimeImplementationWithoutEvidenceClosure,
    RuntimeProofWithoutReceipt,
    SupportPromotionWithoutClosure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePromotionFinding {
    pub code: RuntimePromotionFindingCode,
    #[serde(default)]
    pub requirement_id: Option<RequirementId>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatedRuntimeTransition {
    pub slice_id: ImplementationSliceId,
    pub slice_generation: u32,
    pub requirement_delta: Vec<RequirementDelta>,
    pub implementation_claim_status: ImplementationClaimStatus,
    pub evidence_state: EvidenceDispositionState,
    pub support_claim_state: SupportClaimDispositionState,
}

pub fn validate_runtime_promotion(
    requirements: &RequirementGraph,
    slice: &ImplementationSliceV1,
) -> Vec<RuntimePromotionFinding> {
    let requirement_by_id = requirements
        .requirements
        .iter()
        .map(|requirement| (requirement.id.clone(), requirement))
        .collect::<BTreeMap<_, _>>();
    let mut findings = Vec::new();
    let mut runtime_requirement_ids = Vec::new();

    for delta in &slice.requirement_delta {
        let Some(requirement) = requirement_by_id.get(&delta.requirement_id) else {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::RequirementNotFound,
                Some(delta.requirement_id.clone()),
                format!(
                    "implementation slice references unknown requirement {}",
                    delta.requirement_id.as_str()
                ),
            ));
            continue;
        };

        if delta.requirement_generation != requirement.generation {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::RequirementGenerationMismatch,
                Some(delta.requirement_id.clone()),
                format!(
                    "requirement {} generation {} does not match current generation {}",
                    delta.requirement_id.as_str(),
                    delta.requirement_generation,
                    requirement.generation
                ),
            ));
        }
        if let Some(change) = &delta.status_change {
            if change.to != requirement.status {
                findings.push(RuntimePromotionFinding::new(
                    RuntimePromotionFindingCode::RequirementStatusMismatch,
                    Some(delta.requirement_id.clone()),
                    format!(
                        "requirement {} status change targets {:?}, but current normative status is {:?}",
                        delta.requirement_id.as_str(),
                        change.to,
                        requirement.status
                    ),
                ));
            }
        }
        if delta.runtime {
            runtime_requirement_ids.push((delta.requirement_id.clone(), requirement.status));
        }
    }

    validate_implementation_claim(slice, &runtime_requirement_ids, &mut findings);
    validate_current_evidence(slice, &mut findings);
    validate_support_promotion(slice, &mut findings);

    findings.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.requirement_id.cmp(&right.requirement_id))
            .then_with(|| left.message.cmp(&right.message))
    });
    findings
}

pub fn validated_runtime_transition(
    requirements: &RequirementGraph,
    slice: &ImplementationSliceV1,
) -> Result<ValidatedRuntimeTransition, Vec<RuntimePromotionFinding>> {
    let findings = validate_runtime_promotion(requirements, slice);
    if !findings.is_empty() {
        return Err(findings);
    }

    Ok(ValidatedRuntimeTransition {
        slice_id: slice.id.clone(),
        slice_generation: slice.generation,
        requirement_delta: slice.requirement_delta.clone(),
        implementation_claim_status: slice.implementation_claim.status,
        evidence_state: slice.evidence.state,
        support_claim_state: slice.support_claim.state,
    })
}

fn validate_implementation_claim(
    slice: &ImplementationSliceV1,
    runtime_requirements: &[(RequirementId, RequirementStatus)],
    findings: &mut Vec<RuntimePromotionFinding>,
) {
    if slice.implementation_claim.status != ImplementationClaimStatus::Implemented {
        return;
    }

    for (requirement_id, status) in runtime_requirements {
        if !status.allows_implementation_claim() {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::RequirementStatusDoesNotAllowImplementation,
                Some(requirement_id.clone()),
                format!(
                    "requirement {} has normative status {:?}, which does not allow an ordinary implemented claim",
                    requirement_id.as_str(),
                    status
                ),
            ));
        }
    }

    if slice.change_class == ImplementationSliceClass::SpecOrPolicyChange {
        for (requirement_id, _) in runtime_requirements {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::SpecOnlyRuntimeImplementationClaim,
                Some(requirement_id.clone()),
                format!(
                    "spec or policy slice cannot publish an implemented runtime claim for {}",
                    requirement_id.as_str()
                ),
            ));
        }
        return;
    }

    if !evidence_is_current_and_receipted(slice) {
        for (requirement_id, _) in runtime_requirements {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::RuntimeImplementationWithoutEvidenceClosure,
                Some(requirement_id.clone()),
                format!(
                    "implemented runtime claim for {} requires current receipt-backed evidence",
                    requirement_id.as_str()
                ),
            ));
        }
    }
}

fn validate_current_evidence(
    slice: &ImplementationSliceV1,
    findings: &mut Vec<RuntimePromotionFinding>,
) {
    if slice.evidence.state == EvidenceDispositionState::Current
        && slice
            .evidence
            .receipt
            .as_deref()
            .is_none_or(|receipt| receipt.trim().is_empty())
    {
        findings.push(RuntimePromotionFinding::new(
            RuntimePromotionFindingCode::RuntimeProofWithoutReceipt,
            None,
            "current runtime proof requires a non-empty receipt reference",
        ));
    }
}

fn validate_support_promotion(
    slice: &ImplementationSliceV1,
    findings: &mut Vec<RuntimePromotionFinding>,
) {
    if slice.support_claim.state != SupportClaimDispositionState::Promoted {
        return;
    }

    let implementation_closed =
        slice.implementation_claim.status == ImplementationClaimStatus::Implemented;
    let evidence_closed = evidence_is_current_and_receipted(slice);
    let claim_named = slice
        .support_claim
        .claim
        .as_deref()
        .is_some_and(|claim| !claim.trim().is_empty());

    if !implementation_closed || !evidence_closed || !claim_named {
        findings.push(RuntimePromotionFinding::new(
            RuntimePromotionFindingCode::SupportPromotionWithoutClosure,
            None,
            "runtime support promotion requires an implemented claim, current receipt-backed evidence, and a named support claim",
        ));
    }
}

fn evidence_is_current_and_receipted(slice: &ImplementationSliceV1) -> bool {
    slice.evidence.state == EvidenceDispositionState::Current
        && slice
            .evidence
            .receipt
            .as_deref()
            .is_some_and(|receipt| !receipt.trim().is_empty())
}

impl RuntimePromotionFinding {
    fn new(
        code: RuntimePromotionFindingCode,
        requirement_id: Option<RequirementId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            requirement_id,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_system::{parse_implementation_slice, parse_requirement_blocks};

    const SPEC: &str = r#"---
id: CARGO-ALLOW-SPEC-0009
kind: spec
---

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
status = "accepted"
statement = "A spec-only slice cannot promote runtime state without closure."
claim_class = "runtime_behavior"
```
"#;

    const SLICE: &str = r#"
schema_version = "2.0"
id = "cargo-allow.slice.self-hosted-runtime-promotion.v1"
generation = 1
source_issue = "issue:2206"
design_reference = "design:self-hosted-runtime-promotion"
change_class = "spec_or_policy_change"
basis = "git:example-head"
claim_boundary = "Defines the requirement without claiming runtime completion."

[[requirement_delta]]
requirement_id = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
requirement_generation = 1
runtime = true

[implementation_claim]
status = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;

    fn graph() -> Result<RequirementGraph, String> {
        parse_requirement_blocks(SPEC).map_err(|error| error.to_string())
    }

    fn slice() -> Result<ImplementationSliceV1, String> {
        parse_implementation_slice(SLICE).map_err(|error| error.to_string())
    }

    #[test]
    fn spec_or_policy_slice_keeps_runtime_requirement_accepted() -> Result<(), String> {
        let graph = graph()?;
        let slice = slice()?;

        let transition = validated_runtime_transition(&graph, &slice)
            .map_err(|findings| format!("unexpected findings: {findings:?}"))?;

        assert_eq!(transition.requirement_delta.len(), 1);
        assert_eq!(
            transition.implementation_claim_status,
            ImplementationClaimStatus::Outstanding
        );
        assert_eq!(
            transition.support_claim_state,
            SupportClaimDispositionState::Unchanged
        );
        assert_eq!(
            graph
                .requirements
                .first()
                .map(|requirement| requirement.status),
            Some(RequirementStatus::Accepted)
        );
        Ok(())
    }

    #[test]
    fn spec_or_policy_slice_rejects_unproved_runtime_promotion() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        slice.implementation_claim.status = ImplementationClaimStatus::Implemented;

        let findings = validate_runtime_promotion(&graph, &slice);

        assert_eq!(
            findings,
            vec![RuntimePromotionFinding {
                code: RuntimePromotionFindingCode::SpecOnlyRuntimeImplementationClaim,
                requirement_id: Some(RequirementId(
                    "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion".to_string()
                )),
                message: "spec or policy slice cannot publish an implemented runtime claim for CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion".to_string(),
            }]
        );
        assert!(validated_runtime_transition(&graph, &slice).is_err());
        assert_eq!(
            graph
                .requirements
                .first()
                .map(|requirement| requirement.status),
            Some(RequirementStatus::Accepted)
        );
        Ok(())
    }

    #[test]
    fn spec_or_policy_slice_rejects_invalid_transition_broadly() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        slice.support_claim.state = SupportClaimDispositionState::Promoted;

        // Intentionally broad neighboring control: later test-grip integration
        // must not credit this as the exact support-promotion discriminator.
        assert!(!validate_runtime_promotion(&graph, &slice).is_empty());
        Ok(())
    }

    #[test]
    fn spec_or_policy_slice_rejects_current_proof_without_receipt() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        slice.evidence.state = EvidenceDispositionState::Current;
        let findings = validate_runtime_promotion(&graph, &slice);

        assert_eq!(
            findings.first().map(|finding| finding.code),
            Some(RuntimePromotionFindingCode::RuntimeProofWithoutReceipt)
        );
        Ok(())
    }

    #[test]
    fn behavior_change_accepts_implemented_claim_with_current_evidence() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        slice.change_class = ImplementationSliceClass::BehaviorChange;
        slice.implementation_claim.status = ImplementationClaimStatus::Implemented;
        slice.evidence.state = EvidenceDispositionState::Current;
        slice.evidence.receipt = Some("receipt:current-head".to_string());

        let transition = validated_runtime_transition(&graph, &slice)
            .map_err(|findings| format!("unexpected findings: {findings:?}"))?;
        assert_eq!(
            transition.implementation_claim_status,
            ImplementationClaimStatus::Implemented
        );
        Ok(())
    }

    #[test]
    fn behavior_change_rejects_implemented_claim_without_evidence_closure() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        slice.change_class = ImplementationSliceClass::BehaviorChange;
        slice.implementation_claim.status = ImplementationClaimStatus::Implemented;

        let findings = validate_runtime_promotion(&graph, &slice);
        assert_eq!(
            findings.first().map(|finding| finding.code),
            Some(RuntimePromotionFindingCode::RuntimeImplementationWithoutEvidenceClosure)
        );
        Ok(())
    }

    #[test]
    fn non_accepted_requirement_rejects_ordinary_implemented_claim() -> Result<(), String> {
        let mut graph = graph()?;
        let requirement = graph
            .requirements
            .first_mut()
            .ok_or_else(|| "expected one requirement".to_string())?;
        requirement.status = RequirementStatus::Deferred;

        let mut slice = slice()?;
        slice.change_class = ImplementationSliceClass::BehaviorChange;
        slice.implementation_claim.status = ImplementationClaimStatus::Implemented;
        slice.evidence.state = EvidenceDispositionState::Current;
        slice.evidence.receipt = Some("receipt:current-head".to_string());

        let findings = validate_runtime_promotion(&graph, &slice);
        assert_eq!(
            findings.first().map(|finding| finding.code),
            Some(RuntimePromotionFindingCode::RequirementStatusDoesNotAllowImplementation)
        );
        Ok(())
    }

    #[test]
    fn two_claims_can_reference_one_accepted_requirement() -> Result<(), String> {
        let graph = graph()?;
        let outstanding = slice()?;
        let mut implemented = outstanding.clone();
        implemented.id =
            ImplementationSliceId("cargo-allow.slice.runtime-implementation.v1".into());
        implemented.change_class = ImplementationSliceClass::BehaviorChange;
        implemented.implementation_claim.status = ImplementationClaimStatus::Implemented;
        implemented.evidence.state = EvidenceDispositionState::Current;
        implemented.evidence.receipt = Some("receipt:current-head".into());

        assert!(validate_runtime_promotion(&graph, &outstanding).is_empty());
        assert!(validate_runtime_promotion(&graph, &implemented).is_empty());
        assert_eq!(graph.requirements[0].status, RequirementStatus::Accepted);
        Ok(())
    }
}
