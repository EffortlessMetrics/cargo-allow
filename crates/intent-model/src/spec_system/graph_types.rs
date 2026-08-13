//! Graph compilation DTOs — the stable types consumed by compile_spec_graph (#3520).
//!
//! These types were previously in compiled_graph.rs which was deleted as dead
//! code in #3304. They are restored here as a focused DTO-only module so the
//! graph compiler can move from allow-policy to intent-engine (#3520).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::authored_mapping::{
    EvidenceClaimId, EvidencePurpose, EvidenceSubjectId, EvidenceSubjectRegistration,
    EvidenceSubjectRole, ImplementationSeamId, SourceLocation,
};
use super::implementation_slice::{
    EvidenceDispositionState, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, SupportClaimDispositionState,
};
use super::requirement::{
    RequirementClaimClass, RequirementGraph, RequirementId, RequirementStatus,
};

/// Stable snapshot identifier for a compiled graph.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphSnapshotId(pub String);

impl GraphSnapshotId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSeamRegistration {
    pub id: ImplementationSeamId,
    pub owner: String,
    pub operation: String,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceClaimRegistration {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub slice_id: ImplementationSliceId,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject_ids: Vec<EvidenceSubjectId>,
    #[serde(default)]
    pub related_subject_ids: Vec<EvidenceSubjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementNode {
    pub id: RequirementId,
    pub generation: u32,
    pub status: RequirementStatus,
    pub claim_class: RequirementClaimClass,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSliceNode {
    pub id: ImplementationSliceId,
    pub generation: u32,
    pub change_class: ImplementationSliceClass,
    pub implementation_claim_status: ImplementationClaimStatus,
    pub evidence_state: EvidenceDispositionState,
    pub support_claim_state: SupportClaimDispositionState,
    pub requirement_ids: Vec<RequirementId>,
    pub owned_seams: BTreeSet<String>,
    pub shared_seams: BTreeSet<String>,
    pub forbidden_seams: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSeamNode {
    pub id: ImplementationSeamId,
    pub owner: String,
    pub operation: String,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSubjectNode {
    pub id: EvidenceSubjectId,
    pub role: EvidenceSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
    pub source: SourceLocation,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceClaimNode {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub slice_id: ImplementationSliceId,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject_ids: Vec<EvidenceSubjectId>,
    pub related_subject_ids: Vec<EvidenceSubjectId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiagnosticCode {
    DuplicateId,
    UnknownRequirement,
    UnknownSlice,
    UnknownSeam,
    UnknownSubject,
    EmptyEvidenceSubjects,
    ExactSubjectMarkedWeak,
    RelatedSubjectMarkedExact,
    SliceRequirementGenerationMismatch,
    SeamNotDeclaredBySlice,
    ForbiddenSeam,
}

impl GraphDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateId => "duplicate_id",
            Self::UnknownRequirement => "unknown_requirement",
            Self::UnknownSlice => "unknown_slice",
            Self::UnknownSeam => "unknown_seam",
            Self::UnknownSubject => "unknown_subject",
            Self::EmptyEvidenceSubjects => "empty_evidence_subjects",
            Self::ExactSubjectMarkedWeak => "exact_subject_marked_weak",
            Self::RelatedSubjectMarkedExact => "related_subject_marked_exact",
            Self::SliceRequirementGenerationMismatch => "slice_requirement_generation_mismatch",
            Self::SeamNotDeclaredBySlice => "seam_not_declared_by_slice",
            Self::ForbiddenSeam => "forbidden_seam",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDiagnostic {
    pub code: GraphDiagnosticCode,
    pub subject: String,
    pub message: String,
}

impl GraphDiagnostic {
    pub fn new(
        code: GraphDiagnosticCode,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            subject: subject.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphCompileInput {
    pub requirement_graphs: Vec<RequirementGraph>,
    pub implementation_slices: Vec<ImplementationSliceV1>,
    pub seams: Vec<ImplementationSeamRegistration>,
    pub evidence_claims: Vec<EvidenceClaimRegistration>,
    pub subjects: Vec<EvidenceSubjectRegistration>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSpecGraph {
    pub snapshot_id: GraphSnapshotId,
    pub requirements: BTreeMap<RequirementId, RequirementNode>,
    pub slices: BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    pub seams: BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    pub evidence_claims: BTreeMap<EvidenceClaimId, EvidenceClaimNode>,
    pub subjects: BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    pub diagnostics: Vec<GraphDiagnostic>,
}

impl CompiledSpecGraph {
    pub fn evidence_for_requirement(
        &self,
        requirement_id: &RequirementId,
    ) -> Vec<&EvidenceClaimNode> {
        self.evidence_claims
            .values()
            .filter(|claim| &claim.requirement_id == requirement_id)
            .collect()
    }

    pub fn subjects_for_evidence(
        &self,
        evidence_id: &EvidenceClaimId,
    ) -> Vec<&EvidenceSubjectNode> {
        self.evidence_claims
            .get(evidence_id)
            .into_iter()
            .flat_map(|claim| claim.subject_ids.iter())
            .filter_map(|subject_id| self.subjects.get(subject_id))
            .collect()
    }
}
