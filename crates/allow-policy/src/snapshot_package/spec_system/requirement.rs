//! Requirement domain DTOs (#2584-B).

use serde::{Deserialize, Serialize};

pub const REQUIREMENT_BLOCK_SCHEMA_VERSION: &str = "1.0";
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequirementId(pub String);

impl RequirementId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Normative status of a requirement.
///
/// This deliberately does not contain implementation states. A requirement may
/// remain accepted while different implementation claims are planned,
/// implemented, unsupported, or stale independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Draft,
    Accepted,
    Deferred,
    Superseded,
    Rejected,
    RemovedWithReplacement,
}

impl RequirementStatus {
    pub fn allows_implementation_claim(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementClaimClass {
    RuntimeBehavior,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecRequirement {
    pub id: RequirementId,
    pub local_id: String,
    pub generation: u32,
    pub status: RequirementStatus,
    pub statement: String,
    pub claim_class: RequirementClaimClass,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementSource {
    #[serde(default)]
    pub path: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub content_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementGraph {
    pub schema_version: String,
    pub document_id: String,
    pub source: RequirementSource,
    pub requirements: Vec<SpecRequirement>,
}
