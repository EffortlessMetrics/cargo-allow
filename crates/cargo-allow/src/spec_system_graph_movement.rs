//! Paired graph movement taxonomy aligned with intent-engine (#2586-C).
//!
//! `cargo-allow` retains graph comparison runtime during the parity window.
//! Movement kind strings must stay aligned with `intent-engine::GraphMovementKindV1`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecGraphMovementKind {
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

impl SpecGraphMovementKind {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecGraphMovement {
    pub kind: SpecGraphMovementKind,
    pub id: String,
}

pub const PRECOMMIT_PHASE_ID: &str = "precommit";
