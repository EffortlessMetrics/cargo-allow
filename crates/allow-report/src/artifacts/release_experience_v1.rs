//! Release-facing installed-experience receipt (#3151).
//!
//! The evaluator binds an exact installed candidate's first-hour and
//! finding-to-green coherence claims without recomputing package, scanner,
//! mutation, or release truth. A clean external pilot that never ran cannot
//! become `Complete`: the receipt stays explicitly `NotProven` with narrowed
//! claims until #3150 selects and authorizes a pilot target. This module
//! performs no installation, executes no candidate, and mutates nothing.

use serde::{Deserialize, Serialize};

mod evaluate;
#[cfg(test)]
mod tests;
pub use evaluate::evaluate_release_experience_v1;

pub const RELEASE_EXPERIENCE_SCHEMA_ID: &str = "cargo-allow.release-experience.v1";
pub const RELEASE_EXPERIENCE_SCHEMA_VERSION: u32 = 1;

/// Maximum length of any single human-authored bound string.
pub const RELEASE_EXPERIENCE_MAX_TEXT_LEN: usize = 2000;

/// Docs fixtures the coherence claim must name when it claims completeness.
pub const RELEASE_EXPERIENCE_REQUIRED_DOCS: [&str; 8] = [
    "readme",
    "getting-started",
    "help",
    "completion",
    "manpage",
    "channel",
    "support-matrix",
    "command-registry",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceDocsIdentityV1 {
    pub name: String,
    pub digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExperiencePilotResultV1 {
    Complete,
    Incomplete,
    NotProven,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperiencePilotV1 {
    pub receipt_digest: String,
    pub result: ReleaseExperiencePilotResultV1,
    pub friction_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExperienceBrownfieldPostureV1 {
    IncludedWithReceipt,
    NotIncludedPendingPublishedPilot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExperienceFrictionDispositionV1 {
    Closed,
    AcceptedPostRelease,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceFrictionV1 {
    pub id: String,
    pub disposition: ReleaseExperienceFrictionDispositionV1,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceInputV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub candidate_digest: String,
    pub install_digest: String,
    pub journey_digest: String,
    pub binary_digest: String,
    pub invocation_path: String,
    pub support_matrix_generation: String,
    pub command_registry_generation: String,
    pub migration_denominator_digest: String,
    pub migration_schema_id: String,
    pub clean_pilot: Option<ReleaseExperiencePilotV1>,
    pub brownfield_posture: ReleaseExperienceBrownfieldPostureV1,
    pub brownfield_receipt_digest: Option<String>,
    pub docs_identities: Vec<ReleaseExperienceDocsIdentityV1>,
    pub frictions: Vec<ReleaseExperienceFrictionV1>,
    /// Claimed outcome. `Complete` requires pilot proof; `NotProven`
    /// requires an explicit reason and narrowed claims.
    pub claimed_result: ReleaseExperienceResultV1,
    pub not_proven_reason: String,
    pub narrowed_claims: Vec<String>,
    pub observed_at_unix_seconds: u64,
    pub evaluated_at_unix_seconds: u64,
    pub maximum_age_seconds: u64,
}

/// Ordered by fail-closed aggregate precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExperienceResultV1 {
    Unsupported,
    Malformed,
    InstrumentFailure,
    Mismatch,
    Stale,
    Incomplete,
    NotProven,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceFindingV1 {
    pub result: ReleaseExperienceResultV1,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseExperienceV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub result: ReleaseExperienceResultV1,
    pub findings: Vec<ReleaseExperienceFindingV1>,
    /// What remains proven when the receipt is not Complete (installed
    /// package/install/journey truth is never discarded).
    pub retained_evidence: Vec<String>,
    pub evaluated_at_unix_seconds: u64,
}

/// Declaration order is canonical; consumers must reconcile before trusting fields.
pub fn render_release_experience_v1(
    receipt: &CargoAllowReleaseExperienceV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}
