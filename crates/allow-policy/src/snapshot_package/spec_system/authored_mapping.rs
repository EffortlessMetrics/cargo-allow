//! Authored seam/evidence mapping DTOs (#2584-B).

use serde::{Deserialize, Serialize};

use super::compiled_graph::{
    EvidenceClaimId, EvidencePurpose, ImplementationSeamId, SourceLocation,
};
use super::implementation_slice::ImplementationSliceId;
use super::requirement::RequirementId;

pub const AUTHORED_MAPPING_SCHEMA_VERSION: &str = "1.0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredSeamSource {
    pub schema_version: String,
    #[serde(default)]
    pub seam: Vec<AuthoredSeam>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredSeam {
    pub id: ImplementationSeamId,
    pub generation: u32,
    pub owner: String,
    pub operation: String,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredEvidenceSource {
    pub schema_version: String,
    #[serde(default)]
    pub evidence: Vec<AuthoredEvidenceClaim>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredEvidenceClaim {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub requirement_generation: u32,
    pub slice_id: ImplementationSliceId,
    pub slice_generation: u32,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject: Vec<AuthoredSubjectSelector>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredSubjectSelector {
    pub id: String,
    pub role: AuthoredSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredSubjectRole {
    ExactEvidence,
    RelatedWeak,
}
