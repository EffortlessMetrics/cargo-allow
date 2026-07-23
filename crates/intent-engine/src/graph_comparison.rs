//! Paired graph comparison transport DTOs (#2586-C).
//!
//! These types describe normalized semantic movements between parent and
//! candidate compiled graphs. They do not parse sources or invoke graph
//! compilation.

use serde::{Deserialize, Serialize};

pub const GRAPH_COMPARISON_REPORT_SCHEMA_ID: &str = "intent.graph-comparison-report.v1";

/// Normalized semantic movement between paired compiled spec graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphMovementKindV1 {
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
    ProfileOrDialectChanged,
    UnknownOrUncomparable,
}

impl GraphMovementKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequirementAdded => "requirement_added",
            Self::RequirementRemoved => "requirement_removed",
            Self::RequirementChanged => "requirement_changed",
            Self::ImplementationSliceAdded => "implementation_slice_added",
            Self::ImplementationSliceRemoved => "implementation_slice_removed",
            Self::ImplementationSliceChanged => "implementation_slice_changed",
            Self::SeamMappingAdded => "seam_mapping_added",
            Self::SeamMappingRemoved => "seam_mapping_removed",
            Self::SeamMappingChanged => "seam_mapping_changed",
            Self::EvidencePurposeAdded => "evidence_purpose_added",
            Self::EvidencePurposeRemoved => "evidence_purpose_removed",
            Self::EvidencePurposeChanged => "evidence_purpose_changed",
            Self::EvidenceClaimChanged => "evidence_claim_changed",
            Self::SubjectSelectorAdded => "subject_selector_added",
            Self::SubjectSelectorRemoved => "subject_selector_removed",
            Self::SubjectSelectorChanged => "subject_selector_changed",
            Self::SubjectBodyIdentityChanged => "subject_body_identity_changed",
            Self::ProfileOrDialectChanged => "profile_or_dialect_changed",
            Self::UnknownOrUncomparable => "unknown_or_uncomparable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphMovementV1 {
    pub kind: GraphMovementKindV1,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphComparisonReportV1 {
    pub schema_id: String,
    pub parent_identity: String,
    pub candidate_identity: String,
    pub movements: Vec<GraphMovementV1>,
}

impl GraphComparisonReportV1 {
    pub fn new(
        parent_identity: impl Into<String>,
        candidate_identity: impl Into<String>,
        movements: Vec<GraphMovementV1>,
    ) -> Self {
        Self {
            schema_id: GRAPH_COMPARISON_REPORT_SCHEMA_ID.to_string(),
            parent_identity: parent_identity.into(),
            candidate_identity: candidate_identity.into(),
            movements,
        }
    }
}

/// Canonical movement-kind ordering used by paired graph comparison.
pub fn canonical_graph_movement_kinds() -> &'static [GraphMovementKindV1] {
    const KINDS: &[GraphMovementKindV1] = &[
        GraphMovementKindV1::RequirementAdded,
        GraphMovementKindV1::RequirementRemoved,
        GraphMovementKindV1::RequirementChanged,
        GraphMovementKindV1::ImplementationSliceAdded,
        GraphMovementKindV1::ImplementationSliceRemoved,
        GraphMovementKindV1::ImplementationSliceChanged,
        GraphMovementKindV1::SeamMappingAdded,
        GraphMovementKindV1::SeamMappingRemoved,
        GraphMovementKindV1::SeamMappingChanged,
        GraphMovementKindV1::EvidencePurposeAdded,
        GraphMovementKindV1::EvidencePurposeRemoved,
        GraphMovementKindV1::EvidencePurposeChanged,
        GraphMovementKindV1::EvidenceClaimChanged,
        GraphMovementKindV1::SubjectSelectorAdded,
        GraphMovementKindV1::SubjectSelectorRemoved,
        GraphMovementKindV1::SubjectSelectorChanged,
        GraphMovementKindV1::SubjectBodyIdentityChanged,
        GraphMovementKindV1::ProfileOrDialectChanged,
        GraphMovementKindV1::UnknownOrUncomparable,
    ];
    KINDS
}

pub fn sort_graph_movements(movements: &mut [GraphMovementV1]) {
    movements.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub fn load_graph_comparison_report_json(text: &str) -> Result<GraphComparisonReportV1, String> {
    serde_json::from_str(text).map_err(|err| format!("parse graph comparison report: {err}"))
}
