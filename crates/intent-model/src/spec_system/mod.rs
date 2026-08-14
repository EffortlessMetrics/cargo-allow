//! Spec-system domain types and parsing (#2584-B/C).

mod active_goal;
mod authored_mapping;
mod config;
mod doc_artifacts;
mod graph_types;
mod implementation_slice;
mod import_roots;
mod precommit_types;
mod requirement;
mod support_tiers;

pub use active_goal::{
    ActiveGoalManifest, ActiveGoalStatus, ActiveGoalWorkItem, ActiveGoalWorkItemStatus,
    parse_active_goal_manifest, parse_active_goal_manifest_at, validate_active_goal_manifest,
    validate_active_goal_manifest_text, validate_active_goal_manifest_text_at,
};
pub use authored_mapping::{
    AUTHORED_MAPPING_SCHEMA_VERSION, AuthoredEvidenceClaim, AuthoredEvidenceSource, AuthoredSeam,
    AuthoredSeamSource, AuthoredSubjectRole, AuthoredSubjectSelector, EvidenceClaimId,
    EvidencePurpose, EvidenceSubjectId, EvidenceSubjectRegistration, EvidenceSubjectRole,
    ImplementationSeamId, SourceLocation, parse_authored_evidence, parse_authored_evidence_at,
    parse_authored_seams, parse_authored_seams_at, validate_authored_mapping,
};
pub use config::{
    SpecSystemConfig, SpecSystemGeneration, SpecSystemMode, SpecSystemRequirements,
    SpecSystemRoots, parse_spec_system_config, parse_spec_system_config_at,
};
pub use doc_artifacts::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, load_doc_artifacts,
    parse_doc_artifact_ledger, parse_doc_artifact_ledger_at,
};
pub use graph_types::{
    CompiledSpecGraph, EvidenceClaimNode, EvidenceClaimRegistration, EvidenceSubjectNode,
    GraphCompileInput, GraphDiagnostic, GraphDiagnosticCode, GraphSnapshotId,
    ImplementationSeamNode, ImplementationSeamRegistration, ImplementationSliceNode,
    RequirementNode,
};
pub use implementation_slice::{
    EvidenceDisposition, EvidenceDispositionState, IMPLEMENTATION_SLICE_SCHEMA_VERSION,
    ImplementationClaim, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, RequirementDelta, SupportClaimDisposition,
    SupportClaimDispositionState, parse_implementation_slice, parse_implementation_slice_at,
};
pub use import_roots::{ImportNodeRole, ImportRootEntry, ImportRootsConfig};
pub use precommit_types::{
    PrecommitChangeClass, PrecommitChangeDeclaration, PrecommitEvaluationInput, PrecommitFinding,
    PrecommitFindingCode, PrecommitFindingPosture, PrecommitInventoryPosture, PrecommitMovement,
    PrecommitMovementKind, PrecommitObjectiveEvaluation, PrecommitSubjectResolution,
    PrecommitSubjectResolutionStatus,
};
pub use requirement::{
    REQUIREMENT_BLOCK_SCHEMA_VERSION, RequirementClaimClass, RequirementGraph, RequirementId,
    RequirementSource, RequirementStatus, SpecRequirement, parse_requirement_blocks,
    parse_requirement_blocks_at,
};
pub use support_tiers::{
    SupportTierLevel, SupportTierRow, parse_support_tier_claims, validate_support_tier_claims,
};
