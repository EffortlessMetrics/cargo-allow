use allow_policy::spec_system::{
    EvidenceDispositionState, ImplementationClaimStatus, RequirementStatus,
    SupportClaimDispositionState, parse_implementation_slice_at, parse_requirement_blocks_at,
    validate_runtime_promotion, validated_runtime_transition,
};
use std::path::Path;

const SPEC_PATH: &str = "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md";
const SLICE_PATH: &str = ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml";
const SPEC: &str = include_str!(
    "../../../docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md"
);
const SLICE: &str = include_str!(
    "../../../.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml"
);

#[test]
fn self_hosted_runtime_promotion_sources_parse_and_remain_outstanding() -> Result<(), String> {
    let requirements = parse_requirement_blocks_at(Some(Path::new(SPEC_PATH)), SPEC)
        .map_err(|error| error.to_string())?;
    let slice = parse_implementation_slice_at(Some(Path::new(SLICE_PATH)), SLICE)
        .map_err(|error| error.to_string())?;

    let requirement = requirements
        .requirements
        .first()
        .ok_or_else(|| "expected one self-hosted requirement".to_string())?;
    assert_eq!(requirements.requirements.len(), 1);
    assert_eq!(
        requirement.id.as_str(),
        "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
    );
    assert_eq!(requirement.generation, 1);
    assert_eq!(requirement.status, RequirementStatus::Accepted);
    assert_eq!(requirements.source.path.as_deref(), Some(SPEC_PATH));
    assert!(requirements.source.start_line < requirements.source.end_line);
    assert!(requirements.source.content_identity.starts_with("fnv1a64:"));

    assert_eq!(slice.requirement_delta.len(), 1);
    assert_eq!(
        slice.implementation_claim.status,
        ImplementationClaimStatus::Outstanding
    );
    assert_eq!(slice.evidence.state, EvidenceDispositionState::Outstanding);
    assert_eq!(
        slice.support_claim.state,
        SupportClaimDispositionState::Unchanged
    );
    assert!(!SLICE.contains("basis ="));
    assert!(!SLICE.contains("runtime ="));
    assert!(!SLICE.contains("branch ="));
    assert!(!SLICE.contains("progress ="));

    let findings = validate_runtime_promotion(&requirements, &slice);
    assert!(findings.is_empty(), "unexpected findings: {findings:?}");
    let transition = validated_runtime_transition(&requirements, &slice)
        .map_err(|findings| format!("unexpected findings: {findings:?}"))?;
    assert_eq!(
        transition.implementation_claim_status,
        ImplementationClaimStatus::Outstanding
    );
    assert_eq!(
        transition.evidence_state,
        EvidenceDispositionState::Outstanding
    );
    assert_eq!(
        transition.support_claim_state,
        SupportClaimDispositionState::Unchanged
    );
    Ok(())
}
