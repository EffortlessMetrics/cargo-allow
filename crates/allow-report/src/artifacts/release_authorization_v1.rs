//! Production one-operation release authorization contract (#3789).
//!
//! The compiler validates an exact authorization document against the frozen
//! candidate facts it names. It performs no network access, reads no
//! credentials, creates no tags, uploads nothing, and mutates no live state:
//! modeling authority is not granting it. The actual authorization artifact
//! lives outside the frozen source tree; a document whose digest appears in
//! the frozen file inventory is rejected.

use serde::{Deserialize, Serialize};

use crate::FinalRegistryPreflightResultV1 as PreflightResultV1;

mod evaluate;
#[cfg(test)]
mod tests;
pub use evaluate::{
    compile_release_authorization_v1, release_authorization_denominator_binding_v1,
    transition_authorization_consumption,
};

pub const RELEASE_AUTHORIZATION_SCHEMA_ID: &str = "cargo-allow.release-authorization.v1";
pub const RELEASE_AUTHORIZATION_SCHEMA_VERSION: u32 = 1;

/// The single clean final operation this generation may authorize.
pub const RELEASE_AUTHORIZATION_FINAL_OPERATION: &str = "publish_cargo_allow_final_0_2_0";
/// The single recovery operation this generation may authorize.
pub const RELEASE_AUTHORIZATION_RECOVERY_OPERATION: &str =
    "publish_cargo_allow_recovery_0_2_0";
pub const RELEASE_AUTHORIZATION_FINAL_VERSION: &str = "0.2.0";
pub const RELEASE_AUTHORIZATION_FINAL_TAG: &str = "v0.2.0";
pub const RELEASE_AUTHORIZATION_STABLE_CHANNEL: &str = "stable";
/// Only this authentication class may carry the clean final operation.
pub const RELEASE_AUTHORIZATION_AUTH_CLASS: &str = "crates_io_api_token";
/// Structured maintainer statements longer than this are unbounded prose.
pub const RELEASE_AUTHORIZATION_MAX_STATEMENT_LEN: usize = 2000;

/// Exact final denominator in release order: (logical_id, package, version, shared).
pub const RELEASE_AUTHORIZATION_SELECTION: [(&str, &str, &str, bool); 13] = [
    ("allow-core", "allow-core", "0.2.0", false),
    ("allow-policy", "allow-policy", "0.2.0", false),
    ("allow-inventory", "allow-inventory", "0.2.0", false),
    ("allow-files", "allow-files", "0.2.0", false),
    ("allow-rust", "allow-rust", "0.2.0", false),
    ("allow-match", "allow-match", "0.2.0", false),
    ("allow-report", "allow-report", "0.2.0", false),
    ("allow-policy-legacy", "allow-policy-legacy", "0.2.0", false),
    ("repo-protocol", "effortless-repo-protocol", "0.1.0", true),
    ("repo-snapshot", "effortless-repo-snapshot", "0.1.0", true),
    ("repo-edit", "effortless-repo-edit", "0.1.0", true),
    ("allow-diff", "allow-diff", "0.2.0", false),
    ("cargo-allow", "cargo-allow", "0.2.0", false),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAuthorizationAuthorityKindV1 {
    Clean,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationOperationV1 {
    pub name: String,
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    pub authority_kind: ReleaseAuthorizationAuthorityKindV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationPackageRowV1 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub package_digest: String,
    pub package_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationSharedRowV1 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub expected_checksum: String,
    pub authority_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationFreezeV1 {
    pub receipt_digest: String,
    pub candidate_digest: String,
    pub denominator_digest: String,
    pub commit: String,
    pub tree: String,
    pub lock_digest: String,
    pub topology_id: String,
    pub packages: Vec<ReleaseAuthorizationPackageRowV1>,
    pub shared_prerequisites: Vec<ReleaseAuthorizationSharedRowV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationEvidenceV1 {
    pub package_docs_digest: String,
    pub preflight_result: PreflightResultV1,
    pub preflight_evaluated_at_unix_seconds: u64,
    pub preflight_maximum_age_seconds: u64,
    pub support_digest: String,
    pub manifest_digest: String,
    /// The zero-upload rehearsal is Complete in every phase except the
    /// deliberate authorization hold; anything else cannot be authorized.
    pub rehearsal_complete_except_authorization: bool,
    pub rehearsal_digest: String,
    pub source_controls_digest: String,
    pub live_controls_digest: String,
    pub workflow_digest: String,
    pub action_inventory_digest: String,
    pub observed_context_digest: String,
    pub current_context_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAuthorizationSecretStateV1 {
    Available,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationSecretAvailabilityV1 {
    /// Always true: secret values never travel in authorization documents.
    pub redacted: bool,
    pub state: ReleaseAuthorizationSecretStateV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAuthorizationSourceKindV1 {
    IssueComment,
    WorkflowDispatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationSourceV1 {
    pub kind: ReleaseAuthorizationSourceKindV1,
    pub repository: String,
    pub reference: String,
    pub author: String,
    pub body_digest: String,
    pub statement: String,
}

/// One-use lifecycle. Only `Available` may be selected for a run; every other
/// state is terminal for selection and maps to a receipt result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAuthorizationConsumptionV1 {
    Available,
    SelectedForRun,
    IrreversibleOperationStarted,
    ConsumedComplete,
    ConsumedIncident,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationAuthorityV1 {
    pub selected_auth_class: String,
    pub secret_availability: ReleaseAuthorizationSecretAvailabilityV1,
    pub maintainer_actor: String,
    pub maintainer_role: String,
    pub source: ReleaseAuthorizationSourceV1,
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub one_run_scope: bool,
    pub nonce: String,
    pub prior_consumptions: Vec<String>,
    pub consumption: ReleaseAuthorizationConsumptionV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationInputV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation: ReleaseAuthorizationOperationV1,
    pub freeze: ReleaseAuthorizationFreezeV1,
    pub evidence: ReleaseAuthorizationEvidenceV1,
    pub authority: ReleaseAuthorizationAuthorityV1,
    /// Digest inventory of the frozen tree. A document whose digest appears
    /// here authorized itself and is rejected.
    pub frozen_file_digests: Vec<String>,
    pub evaluated_at_unix_seconds: u64,
}

/// Ordered by fail-closed aggregate precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAuthorizationResultV1 {
    Unsupported,
    Malformed,
    InstrumentFailure,
    Unauthorized,
    Mismatch,
    Expired,
    Reused,
    Stale,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorizationFindingV1 {
    pub result: ReleaseAuthorizationResultV1,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseAuthorizationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub result: ReleaseAuthorizationResultV1,
    pub findings: Vec<ReleaseAuthorizationFindingV1>,
    /// Non-blocking residual risks (e.g. unproven registry permission).
    /// A Complete receipt with caveats is authority with eyes open.
    pub caveats: Vec<String>,
    /// Canonical digest of the compiled authorization document.
    pub authorization_digest: String,
    pub evaluated_at_unix_seconds: u64,
}

/// Declaration order is canonical; consumers must reconcile before trusting fields.
pub fn render_release_authorization_v1(
    receipt: &CargoAllowReleaseAuthorizationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}
