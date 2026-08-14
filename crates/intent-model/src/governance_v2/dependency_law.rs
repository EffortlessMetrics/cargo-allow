//! V2 dependency-law DTOs (#2942 step 3 / #3329).
//!
//! Authored dependency law over logical crate identities: forbidden and
//! required edges that closure validation enforces from the V2 authority
//! instead of static parity arrays.

use serde::{Deserialize, Serialize};

/// A forbidden dependency between two logical crates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceForbiddenEdgeV2 {
    pub from_logical_id: String,
    pub to_logical_id: String,
    #[serde(default)]
    pub repair_hint: Option<String>,
}

impl GovernanceForbiddenEdgeV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.from_logical_id.trim().is_empty() || self.to_logical_id.trim().is_empty() {
            return Err("forbidden edge requires non-empty from/to logical ids".into());
        }
        Ok(())
    }
}

/// A required dependency between two logical crates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceRequiredEdgeV2 {
    pub from_logical_id: String,
    pub to_logical_id: String,
    #[serde(default)]
    pub rationale_issue: Option<u32>,
}

impl GovernanceRequiredEdgeV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.from_logical_id.trim().is_empty() || self.to_logical_id.trim().is_empty() {
            return Err("required edge requires non-empty from/to logical ids".into());
        }
        Ok(())
    }
}
