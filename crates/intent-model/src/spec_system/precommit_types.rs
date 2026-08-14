//! Pre-commit evaluation DTOs (#3521 / #2935 slice 5b).
//!
//! These types were previously in compiled_graph.rs (deleted in #3304 as dead
//! code) and lived only in allow-policy's snapshot_package copy. Restored here
//! as a focused DTO module so the precommit evaluator can move from allow-policy
//! to intent-engine.

use super::authored_mapping::EvidenceSubjectId;
use super::graph_types::CompiledSpecGraph;
use super::implementation_slice::{ImplementationSliceId, ImplementationSliceV1};

/// The stable change vocabulary consumed by the pre-commit policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrecommitChangeClass {
    BehaviorChange,
    BugFix,
    RefactorNoIntendedBehaviorChange,
    SpecOrPolicyChange,
    TestOnlyChange,
    GeneratedArtifactChange,
    DocsOnly,
    ToolingOrCiChange,
    DependencyOrToolchainChange,
    ResearchOrEvidenceOnly,
    Mechanical,
    UnknownOrMixed,
}

impl PrecommitChangeClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BehaviorChange => "behavior_change",
            Self::BugFix => "bug_fix",
            Self::RefactorNoIntendedBehaviorChange => "refactor_no_intended_behavior_change",
            Self::SpecOrPolicyChange => "spec_or_policy_change",
            Self::TestOnlyChange => "test_only_change",
            Self::GeneratedArtifactChange => "generated_artifact_change",
            Self::DocsOnly => "docs_only",
            Self::ToolingOrCiChange => "tooling_or_ci_change",
            Self::DependencyOrToolchainChange => "dependency_or_toolchain_change",
            Self::ResearchOrEvidenceOnly => "research_or_evidence_only",
            Self::Mechanical => "mechanical",
            Self::UnknownOrMixed => "unknown_or_mixed",
        }
    }
}

/// A normalized semantic movement from the exact parent to the staged graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrecommitMovementKind {
    RequirementAdded,
    RequirementRemoved,
    RequirementChanged,
    ImplementationSliceAdded,
    ImplementationSliceRemoved,
    ImplementationSliceChanged,
    SeamMappingAdded,
    SeamMappingRemoved,
    SeamMappingChanged,
    EvidencePurposeAdded,
    EvidencePurposeRemoved,
    EvidencePurposeChanged,
    EvidenceClaimChanged,
    SubjectSelectorAdded,
    SubjectSelectorRemoved,
    SubjectSelectorChanged,
    SubjectBodyIdentityChanged,
    GeneratedSourceRelationAdded,
    GeneratedSourceRelationRemoved,
    GeneratedSourceRelationChanged,
    ProfileOrDialectChanged,
    UnknownOrUncomparable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrecommitMovement {
    pub kind: PrecommitMovementKind,
    pub id: String,
}

/// A caller-provided declaration of the change being evaluated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PrecommitChangeDeclaration {
    pub class: Option<PrecommitChangeClass>,
    pub implementation_slice_ids: Vec<ImplementationSliceId>,
    pub regression_subject_ids: Vec<EvidenceSubjectId>,
    pub changed_subject_ids: Vec<EvidenceSubjectId>,
    pub no_intended_behavior_change: bool,
    pub evidence_closure_reviewed: bool,
    pub generated_source_relation_present: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrecommitSubjectResolutionStatus {
    Exact,
    Missing,
    Ambiguous,
    Partial,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrecommitSubjectResolution {
    pub id: EvidenceSubjectId,
    pub status: PrecommitSubjectResolutionStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrecommitInventoryPosture {
    Complete,
    Partial,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrecommitFindingPosture {
    Blocking,
    Advisory,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrecommitFindingCode {
    ChangeClassMissingOrConflicting,
    BehaviorSliceMissing,
    RequirementUnknownOrStale,
    ImplementationClaimInvalid,
    SpecOnlyRuntimePromotion,
    SeamMissingOrForbidden,
    EvidencePurposeMissing,
    ExactSelectorMissing,
    ExactSelectorAmbiguous,
    TestBodyIdentityStale,
    BugFixRegressionMissing,
    TestOnlySubjectUnowned,
    GeneratedSourceRelationMissing,
    SupportClaimAheadOfClosure,
    UnknownStagedSurface,
    InventoryPartialOrUnsupported,
}

impl PrecommitFindingCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChangeClassMissingOrConflicting => {
                "precommit_change_class_missing_or_conflicting"
            }
            Self::BehaviorSliceMissing => "precommit_behavior_slice_missing",
            Self::RequirementUnknownOrStale => "precommit_requirement_unknown_or_stale",
            Self::ImplementationClaimInvalid => "precommit_implementation_claim_invalid",
            Self::SpecOnlyRuntimePromotion => "precommit_spec_only_runtime_promotion",
            Self::SeamMissingOrForbidden => "precommit_seam_missing_or_forbidden",
            Self::EvidencePurposeMissing => "precommit_evidence_purpose_missing",
            Self::ExactSelectorMissing => "precommit_exact_selector_missing",
            Self::ExactSelectorAmbiguous => "precommit_exact_selector_ambiguous",
            Self::TestBodyIdentityStale => "precommit_test_body_identity_stale",
            Self::BugFixRegressionMissing => "precommit_bug_fix_regression_missing",
            Self::TestOnlySubjectUnowned => "precommit_test_only_subject_unowned",
            Self::GeneratedSourceRelationMissing => "precommit_generated_source_relation_missing",
            Self::SupportClaimAheadOfClosure => "precommit_support_claim_ahead_of_closure",
            Self::UnknownStagedSurface => "precommit_unknown_staged_surface",
            Self::InventoryPartialOrUnsupported => "precommit_inventory_partial_or_unsupported",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrecommitFinding {
    pub code: PrecommitFindingCode,
    pub subject: String,
    pub posture: PrecommitFindingPosture,
    pub message: String,
    pub action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrecommitObjectiveEvaluation {
    pub change_class: PrecommitChangeClass,
    pub findings: Vec<PrecommitFinding>,
}

pub struct PrecommitEvaluationInput<'a> {
    pub candidate: &'a CompiledSpecGraph,
    pub slices: &'a [ImplementationSliceV1],
    pub movements: &'a [PrecommitMovement],
    pub declaration: &'a PrecommitChangeDeclaration,
    pub subject_resolutions: &'a [PrecommitSubjectResolution],
    pub inventory: PrecommitInventoryPosture,
    pub legacy_baseline: bool,
}
