//! Compiled spec graph DTOs (#2584-B).

use allow_core::normalize_path;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::implementation_slice::{
    EvidenceDispositionState, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, SupportClaimDispositionState,
};
use super::requirement::{
    RequirementClaimClass, RequirementGraph, RequirementId, RequirementStatus,
};

macro_rules! graph_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

graph_id!(GraphSnapshotId);
graph_id!(ImplementationSeamId);
graph_id!(EvidenceClaimId);
graph_id!(EvidenceSubjectId);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLocation {
    pub path: String,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub symbol: Option<String>,
}

impl SourceLocation {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: normalize_path(path.into()),
            line: None,
            symbol: None,
        }
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePurpose {
    PositiveAcceptance,
    ForbiddenRuntimePromotion,
}

impl EvidencePurpose {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PositiveAcceptance => "positive_acceptance",
            Self::ForbiddenRuntimePromotion => "forbidden_runtime_promotion",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSubjectRole {
    ExactEvidence,
    RelatedWeak,
}

impl EvidenceSubjectRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ExactEvidence => "exact_evidence",
            Self::RelatedWeak => "related_weak",
        }
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
pub struct EvidenceSubjectRegistration {
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
    pub(crate) fn as_str(self) -> &'static str {
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
    pub(crate) fn new(
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

    pub fn related_subjects_for_evidence(
        &self,
        evidence_id: &EvidenceClaimId,
    ) -> Vec<&EvidenceSubjectNode> {
        self.evidence_claims
            .get(evidence_id)
            .into_iter()
            .flat_map(|claim| claim.related_subject_ids.iter())
            .filter_map(|subject_id| self.subjects.get(subject_id))
            .collect()
    }

    pub fn diagnostics_for_slice(&self, slice_id: &ImplementationSliceId) -> Vec<&GraphDiagnostic> {
        let requirement_ids = self
            .slices
            .get(slice_id)
            .map(|slice| {
                slice
                    .requirement_ids
                    .iter()
                    .map(RequirementId::as_str)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        self.diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.subject == slice_id.as_str()
                    || requirement_ids.contains(diagnostic.subject.as_str())
            })
            .collect()
    }
}
