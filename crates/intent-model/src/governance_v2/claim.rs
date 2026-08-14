//! V2 claim boundary DTOs (#2942 step 1 / #3327).
//!
//! Pure authored claim boundary and limitation facts attached to governance
//! records.

use serde::{Deserialize, Serialize};

/// A claim boundary with explicit limitations for a governance record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimBoundaryV2 {
    pub claim: String,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl ClaimBoundaryV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.claim.trim().is_empty() {
            return Err("claim boundary requires a non-empty claim".into());
        }
        Ok(())
    }
}
