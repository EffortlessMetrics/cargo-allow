use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    EvidenceDispositionState, ImplementationDispositionState, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, RequirementDelta, RequirementGraph,
    RequirementId, RequirementLifecycle, SupportClaimDispositionState,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePromotionFindingCode {
    RequirementNotFound,
    RequirementGenerationMismatch,
    RequirementLifecycleMismatch,
    RuntimeImplementationWithoutDisposition,
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
    pub implementation_state: ImplementationDispositionState,
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
        if let Some(from) = delta.from {
            if from != requirement.lifecycle {
                findings.push(RuntimePromotionFinding::new(
                    RuntimePromotionFindingCode::RequirementLifecycleMismatch,
                    Some(delta.requirement_id.clone()),
                    format!(
                        "requirement {} expected lifecycle {:?}, found {:?}",
                        delta.requirement_id.as_str(),
                        from,
                        requirement.lifecycle
                    ),
                ));
            }
        }
    }

    if slice.change_class == ImplementationSliceClass::SpecOrPolicyChange {
        validate_spec_or_policy_promotion(slice, &mut findings);
    }

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
        implementation_state: slice.implementation.state,
        evidence_state: slice.evidence.state,
        support_claim_state: slice.support_claim.state,
    })
}

fn validate_spec_or_policy_promotion(
    slice: &ImplementationSliceV1,
    findings: &mut Vec<RuntimePromotionFinding>,
) {
    for delta in &slice.requirement_delta {
        if delta.runtime
            && delta.to == RequirementLifecycle::Implemented
            && slice.implementation.state != ImplementationDispositionState::Implemented
        {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::RuntimeImplementationWithoutDisposition,
                Some(delta.requirement_id.clone()),
                format!(
                    "spec or policy slice cannot mark runtime requirement {} implemented while implementation remains outstanding",
                    delta.requirement_id.as_str()
                ),
            ));
        }
    }

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
            "spec or policy slice cannot claim current runtime proof without a non-empty receipt reference",
        ));
    }

    if slice.support_claim.state == SupportClaimDispositionState::Promoted {
        let implementation_closed =
            slice.implementation.state == ImplementationDispositionState::Implemented;
        let evidence_closed = slice.evidence.state == EvidenceDispositionState::Current
            && slice
                .evidence
                .receipt
                .as_deref()
                .is_some_and(|receipt| !receipt.trim().is_empty());
        let claim_named = slice
            .support_claim
            .claim
            .as_deref()
            .is_some_and(|claim| !claim.trim().is_empty());

        if !implementation_closed || !evidence_closed || !claim_named {
            findings.push(RuntimePromotionFinding::new(
                RuntimePromotionFindingCode::SupportPromotionWithoutClosure,
                None,
                "spec or policy slice cannot promote runtime support without implemented behavior, current receipt-backed evidence, and a named support claim",
            ));
        }
    }
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
lifecycle = "accepted"
statement = "A spec-only slice cannot promote runtime state without closure."
claim_class = "runtime_behavior"
```
"#;

    const SLICE: &str = r#"
schema_version = "1.0"
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
from = "accepted"
to = "accepted"

[implementation]
state = "outstanding"

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
            transition.requirement_delta.first().map(|delta| delta.to),
            Some(RequirementLifecycle::Accepted)
        );
        assert_eq!(
            transition.implementation_state,
            ImplementationDispositionState::Outstanding
        );
        assert_eq!(
            transition.support_claim_state,
            SupportClaimDispositionState::Unchanged
        );
        Ok(())
    }

    #[test]
    fn spec_or_policy_slice_rejects_unproved_runtime_promotion() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        let delta = slice
            .requirement_delta
            .first_mut()
            .ok_or_else(|| "expected one requirement delta".to_string())?;
        delta.to = RequirementLifecycle::Implemented;

        let findings = validate_runtime_promotion(&graph, &slice);

        assert_eq!(
            findings,
            vec![RuntimePromotionFinding {
                code: RuntimePromotionFindingCode::RuntimeImplementationWithoutDisposition,
                requirement_id: Some(RequirementId(
                    "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion".to_string()
                )),
                message: "spec or policy slice cannot mark runtime requirement CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion implemented while implementation remains outstanding".to_string(),
            }]
        );
        assert!(validated_runtime_transition(&graph, &slice).is_err());
        assert_eq!(
            graph
                .requirements
                .first()
                .map(|requirement| requirement.lifecycle),
            Some(RequirementLifecycle::Accepted)
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
    fn behavior_change_is_not_subject_to_spec_only_rule() -> Result<(), String> {
        let graph = graph()?;
        let mut slice = slice()?;
        slice.change_class = ImplementationSliceClass::BehaviorChange;
        let delta = slice
            .requirement_delta
            .first_mut()
            .ok_or_else(|| "expected one requirement delta".to_string())?;
        delta.to = RequirementLifecycle::Implemented;

        assert!(validate_runtime_promotion(&graph, &slice).is_empty());
        Ok(())
    }
}
