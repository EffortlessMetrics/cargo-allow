//! Implementation slice domain DTOs (#2584-B).

use serde::{Deserialize, Serialize};

use super::requirement::RequirementId;

pub const IMPLEMENTATION_SLICE_SCHEMA_VERSION: &str = "2.0";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImplementationSliceId(pub String);

impl ImplementationSliceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationSliceClass {
    SpecOrPolicyChange,
    BehaviorChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationClaimStatus {
    Outstanding,
    Partial,
    Implemented,
    Unsupported,
    NotApplicable,
    Removed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationClaim {
    pub status: ImplementationClaimStatus,
    #[serde(default)]
    pub seams: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDispositionState {
    Outstanding,
    Current,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDisposition {
    pub state: EvidenceDispositionState,
    #[serde(default)]
    pub receipt: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportClaimDispositionState {
    Unchanged,
    Promoted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportClaimDisposition {
    pub state: SupportClaimDispositionState,
    #[serde(default)]
    pub claim: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementDelta {
    pub requirement_id: RequirementId,
    pub requirement_generation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSliceV1 {
    pub schema_version: String,
    pub id: ImplementationSliceId,
    pub generation: u32,
    pub source_issue: String,
    pub design_reference: String,
    pub change_class: ImplementationSliceClass,
    pub requirement_delta: Vec<RequirementDelta>,
    pub implementation_claim: ImplementationClaim,
    pub evidence: EvidenceDisposition,
    pub support_claim: SupportClaimDisposition,
    #[serde(default)]
    pub owned_seams: Vec<String>,
    #[serde(default)]
    pub shared_seams: Vec<String>,
    #[serde(default)]
    pub forbidden_seams: Vec<String>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default)]
    pub return_conditions: Vec<String>,
    pub claim_boundary: String,
}
