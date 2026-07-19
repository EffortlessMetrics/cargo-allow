use allow_policy::spec_system::{
    EvidencePurpose, ImplementationClaimStatus, RequirementStatus, parse_authored_evidence_at,
    parse_authored_seams_at, parse_implementation_slice_at, parse_requirement_blocks_at,
    validate_authored_mapping,
};
use std::path::Path;

const SPEC_PATH: &str = "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md";
const SLICE_PATH: &str = ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml";
const SEAMS_PATH: &str = ".allow/spec-system/seams/runtime-promotion-validator-v1.toml";
const EVIDENCE_PATH: &str = ".allow/spec-system/evidence/runtime-promotion-v1.toml";

const SPEC: &str =
    include_str!("../../../docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md");
const SLICE: &str =
    include_str!("../../../.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml");
const SEAMS: &str =
    include_str!("../../../.allow/spec-system/seams/runtime-promotion-validator-v1.toml");
const EVIDENCE: &str =
    include_str!("../../../.allow/spec-system/evidence/runtime-promotion-v1.toml");

#[test]
fn retained_self_hosted_mapping_compiles_against_requirement_and_slice() -> Result<(), String> {
    let requirements = parse_requirement_blocks_at(Some(Path::new(SPEC_PATH)), SPEC)
        .map_err(|error| error.to_string())?;
    let slice = parse_implementation_slice_at(Some(Path::new(SLICE_PATH)), SLICE)
        .map_err(|error| error.to_string())?;
    let seams = parse_authored_seams_at(Some(Path::new(SEAMS_PATH)), SEAMS)
        .map_err(|error| error.to_string())?;
    let evidence = parse_authored_evidence_at(Some(Path::new(EVIDENCE_PATH)), EVIDENCE)
        .map_err(|error| error.to_string())?;

    validate_authored_mapping(&requirements, &slice, &seams, &evidence)
        .map_err(|error| error.to_string())?;

    assert_eq!(requirements.requirements.len(), 1);
    assert_eq!(
        requirements.requirements[0].status,
        RequirementStatus::Accepted
    );
    assert_eq!(
        slice.implementation_claim.status,
        ImplementationClaimStatus::Outstanding
    );
    assert_eq!(seams.seam.len(), 1);
    assert_eq!(evidence.evidence.len(), 2);
    assert_eq!(
        evidence.evidence[0].purpose,
        EvidencePurpose::PositiveAcceptance
    );
    assert_eq!(
        evidence.evidence[1].purpose,
        EvidencePurpose::ForbiddenRuntimePromotion
    );
    assert_eq!(evidence.evidence[1].subject.len(), 2);
    assert_eq!(
        evidence.evidence[1].subject[1].role,
        allow_policy::spec_system::AuthoredSubjectRole::RelatedWeak
    );
    Ok(())
}

#[test]
fn retained_mapping_rejects_unknown_or_mismatched_generations() -> Result<(), String> {
    let requirements = parse_requirement_blocks_at(Some(Path::new(SPEC_PATH)), SPEC)
        .map_err(|error| error.to_string())?;
    let slice = parse_implementation_slice_at(Some(Path::new(SLICE_PATH)), SLICE)
        .map_err(|error| error.to_string())?;
    let seams = parse_authored_seams_at(Some(Path::new(SEAMS_PATH)), SEAMS)
        .map_err(|error| error.to_string())?;

    let unknown_requirement = EVIDENCE.replace(
        "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion",
        "CARGO-ALLOW-SPEC-0009#unknown",
    );
    let evidence = parse_authored_evidence(&unknown_requirement)?;
    assert!(validate_authored_mapping(&requirements, &slice, &seams, &evidence).is_err());

    let mismatched_slice = EVIDENCE.replace("slice_generation = 1", "slice_generation = 2");
    let evidence = parse_authored_evidence(&mismatched_slice)?;
    assert!(validate_authored_mapping(&requirements, &slice, &seams, &evidence).is_err());
    Ok(())
}

fn parse_authored_evidence(
    input: &str,
) -> Result<allow_policy::spec_system::AuthoredEvidenceSource, String> {
    parse_authored_evidence_at(None, input).map_err(|error| error.to_string())
}
