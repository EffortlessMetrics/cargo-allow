//! Internal model for the opt-in spec-system profile.
//!
//! This module parses source-tree configuration and artifact ledgers, then
//! validates registered artifact file existence, roots, visible IDs,
//! ledger-resolvable graph links, active-goal TOML references, support-tier
//! claim/proof fields, and bounded requirement/implementation claims. Normative
//! requirement status and implementation-claim status are intentionally
//! independent; compatibility accepts the old field name only when its value
//! remains a normative status. The compiled claim graph connects requirements,
//! PR-local slices, declared seams, evidence purposes, and exact or related
//! subjects without executing proof commands or affecting default cargo-allow
//! behavior.

mod active_goal;
mod authored_mapping;
mod compiled_graph;
mod config;
mod doc_artifacts;
mod implementation_slice;
mod precommit;
mod profile_resolution;
mod requirement;
mod requirement_adapter;
mod ripr_dialect;
mod runtime_promotion;
mod support_tiers;
mod validate;

pub use active_goal::{
    ActiveGoalManifest, ActiveGoalStatus, ActiveGoalWorkItem, ActiveGoalWorkItemStatus,
    parse_active_goal_manifest, parse_active_goal_manifest_at, validate_active_goal_manifest,
    validate_active_goal_manifest_text, validate_active_goal_manifest_text_at,
};
pub use authored_mapping::{
    AUTHORED_MAPPING_SCHEMA_VERSION, AuthoredEvidenceClaim, AuthoredEvidenceSource, AuthoredSeam,
    AuthoredSeamSource, AuthoredSubjectRole, AuthoredSubjectSelector, parse_authored_evidence,
    parse_authored_evidence_at, parse_authored_seams, parse_authored_seams_at,
    validate_authored_mapping,
};
pub use compiled_graph::{
    CompiledSpecGraph, EvidenceClaimId, EvidenceClaimNode, EvidenceClaimRegistration,
    EvidencePurpose, EvidenceSubjectId, EvidenceSubjectNode, EvidenceSubjectRegistration,
    EvidenceSubjectRole, GraphCompileInput, GraphDiagnostic, GraphDiagnosticCode, GraphSnapshotId,
    ImplementationSeamId, ImplementationSeamNode, ImplementationSeamRegistration,
    ImplementationSliceNode, RequirementNode, SourceLocation, compile_spec_graph,
};
pub use config::{
    SpecSystemConfig, SpecSystemGeneration, SpecSystemMode, SpecSystemRequirements,
    SpecSystemRoots, parse_spec_system_config, parse_spec_system_config_at,
};
pub use doc_artifacts::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, load_doc_artifacts,
    parse_doc_artifact_ledger, parse_doc_artifact_ledger_at,
};
pub use implementation_slice::{
    EvidenceDisposition, EvidenceDispositionState, IMPLEMENTATION_SLICE_SCHEMA_VERSION,
    ImplementationClaim, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, RequirementDelta, SupportClaimDisposition,
    SupportClaimDispositionState, parse_implementation_slice, parse_implementation_slice_at,
};
pub use precommit::{
    PrecommitChangeClass, PrecommitChangeDeclaration, PrecommitEvaluationInput, PrecommitFinding,
    PrecommitFindingCode, PrecommitFindingPosture, PrecommitInventoryPosture, PrecommitMovement,
    PrecommitMovementKind, PrecommitObjectiveEvaluation, PrecommitSubjectResolution,
    PrecommitSubjectResolutionStatus, evaluate_precommit_objectives,
};
pub use profile_resolution::{
    ALLOW_CONFIG_REL_PATH, ProfileConfigProvenance, ResolvedProfileConfig, allow_profile_rel_path,
    legacy_profile_rel_path, profile_config_conflict_message, resolve_profile_config,
};
pub use requirement::{
    REQUIREMENT_BLOCK_SCHEMA_VERSION, RequirementClaimClass, RequirementGraph, RequirementId,
    RequirementSource, RequirementStatus, SpecRequirement, parse_requirement_blocks,
    parse_requirement_blocks_at,
};
pub use requirement_adapter::{
    parse_requirement_blocks_for_document, parse_requirement_blocks_for_document_at,
};
pub use ripr_dialect::{
    RIPR_SPEC_DIALECT_ID, RiprSpecDocument, RiprSpecLinks, RiprSpecSource, RiprSpecSourceClass,
    RiprSpecStatus, parse_ripr_implementation_slice, parse_ripr_implementation_slice_at,
    parse_ripr_spec, parse_ripr_spec_at,
};
pub use runtime_promotion::{
    RuntimePromotionFinding, RuntimePromotionFindingCode, ValidatedRuntimeTransition,
    validate_runtime_promotion, validated_runtime_transition,
};
pub use support_tiers::{
    SupportTierLevel, SupportTierRow, parse_support_tier_claims, validate_support_tier_claims,
};
pub use validate::{validate_doc_artifact_files, validate_doc_artifact_links};

#[cfg(test)]
mod design_package_tests;
#[cfg(test)]
mod tests;
