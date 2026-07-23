//! Spec-system domain DTOs (#2584-B).
//!
//! Parsing, validation, and graph compilation remain in `allow-policy` until #2584-C.
#![expect(
    dead_code,
    reason = "policy:allow-9066: spec-system DTO helpers consumed by allow-policy snapshot impl (#2584-B)"
)]

mod active_goal;
mod authored_mapping;
mod compiled_graph;
mod config;
mod doc_artifacts;
mod implementation_slice;
mod import_roots;
mod precommit;
mod requirement;
mod support_tiers;

pub use active_goal::{
    ActiveGoalManifest, ActiveGoalStatus, ActiveGoalWorkItem, ActiveGoalWorkItemStatus,
};
pub use authored_mapping::{
    AUTHORED_MAPPING_SCHEMA_VERSION, AuthoredEvidenceClaim, AuthoredEvidenceSource, AuthoredSeam,
    AuthoredSeamSource, AuthoredSubjectRole, AuthoredSubjectSelector,
};
pub use compiled_graph::{
    CompiledSpecGraph, EvidenceClaimId, EvidenceClaimNode, EvidenceClaimRegistration,
    EvidencePurpose, EvidenceSubjectId, EvidenceSubjectNode, EvidenceSubjectRegistration,
    EvidenceSubjectRole, GraphCompileInput, GraphDiagnostic, GraphDiagnosticCode, GraphSnapshotId,
    ImplementationSeamId, ImplementationSeamNode, ImplementationSeamRegistration,
    ImplementationSliceNode, RequirementNode, SourceLocation,
};
pub use config::{
    SpecSystemConfig, SpecSystemGeneration, SpecSystemMode, SpecSystemRequirements, SpecSystemRoots,
};
pub use doc_artifacts::{ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger};
pub use implementation_slice::{
    EvidenceDisposition, EvidenceDispositionState, IMPLEMENTATION_SLICE_SCHEMA_VERSION,
    ImplementationClaim, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, RequirementDelta, SupportClaimDisposition,
    SupportClaimDispositionState,
};
pub use import_roots::{ImportNodeRole, ImportRootEntry, ImportRootsConfig};
pub use precommit::{
    PrecommitChangeClass, PrecommitChangeDeclaration, PrecommitEvaluationInput, PrecommitFinding,
    PrecommitFindingCode, PrecommitFindingPosture, PrecommitInventoryPosture, PrecommitMovement,
    PrecommitMovementKind, PrecommitObjectiveEvaluation, PrecommitSubjectResolution,
    PrecommitSubjectResolutionStatus,
};
pub use requirement::{
    REQUIREMENT_BLOCK_SCHEMA_VERSION, RequirementClaimClass, RequirementGraph, RequirementId,
    RequirementSource, RequirementStatus, SpecRequirement,
};
pub use support_tiers::{SupportTierLevel, SupportTierRow};
