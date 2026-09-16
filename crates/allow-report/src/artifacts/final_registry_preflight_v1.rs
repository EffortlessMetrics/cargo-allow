//! Pure final-registry feasibility contract (#3849).
//!
//! Provider evidence is an explicit input. This module cannot contact a
//! registry, inspect credentials, authorize publication, or prove that supplied
//! observations were actually collected. Fixture provenance remains visible.

use serde::{Deserialize, Serialize};

mod evaluate;
#[cfg(test)]
mod tests;
pub use evaluate::{evaluate_final_registry_preflight_v1, final_registry_bindings_v1};

/// Retained registry namespace authority, independent of locally packaged bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistrySharedAuthorityV1 {
    pub package_name: String,
    pub package_version: String,
    pub expected_checksum: String,
    pub authority_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryPreflightInputV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub candidate: super::package_candidate_v2::PackageCandidatePayloadV2,
    pub shared_authorities: Vec<FinalRegistrySharedAuthorityV1>,
    pub observed_context: FinalRegistryContextV1,
    pub current_context: FinalRegistryContextV1,
    pub evaluated_at_unix_seconds: u64,
    pub maximum_age_seconds: u64,
    /// Full selected release order, including interleaved shared prerequisites.
    pub observations: Vec<FinalRegistryObservationV1>,
}

pub const FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID: &str = "cargo-allow.final-registry-preflight.v1";
pub const FINAL_REGISTRY_PREFLIGHT_SCHEMA_VERSION: u32 = 1;

/// Every context component invalidates observations independently when moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryContextV1 {
    pub candidate_digest: String,
    pub denominator_digest: String,
    pub workflow_digest: String,
    pub principal: String,
    pub environment: String,
    pub owner_team_digest: String,
    pub release_controls_digest: String,
    pub provider_state_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryRowRoleV1 {
    FinalUploadCandidate,
    SharedPrerequisite,
}

/// Expected checksums are retained independently of provider responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryExpectedRowV1 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub release_order: u32,
    pub role: FinalRegistryRowRoleV1,
    pub expected_checksum: String,
    pub checksum_authority_digest: String,
    /// Shared-row repackaging is diagnostic, never an expected-byte authority.
    pub diagnostic_local_checksum: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryObservationOriginV1 {
    TestFixture,
    ExternalProvider,
}

/// An evidence reference is provenance, not cryptographic authentication of a
/// remote service. The collecting adapter owns retaining the referenced bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryProvenanceV1 {
    pub origin: FinalRegistryObservationOriginV1,
    pub provider: String,
    pub source: String,
    pub evidence_digest: String,
    pub observed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FinalRegistryVersionResponseV1 {
    Found { checksum: String, yanked: bool },
    Missing {},
    NameUnavailable {},
    VisibilityPending {},
    Timeout {},
    RateLimited {},
    ProviderUnavailable {},
    MalformedResponse {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryOwnerStateV1 {
    OwnedByExpectedPrincipal,
    UnexpectedOwner,
    PermissionNotProven,
    ProviderUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryPublishAuthorityV1 {
    Proven,
    SupportingEvidenceOnly,
    NotProven,
    Conflict,
    ProviderUnavailable,
    InstrumentFailure,
}

/// Version, ownership, and authority retain separate provenance and outcomes.
/// A successful version query must not conceal a failed owner endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryObservationV1 {
    pub package_name: String,
    pub package_version: String,
    pub version: FinalRegistryVersionResponseV1,
    pub version_provenance: Option<FinalRegistryProvenanceV1>,
    pub owner: FinalRegistryOwnerStateV1,
    pub owner_provenance: Option<FinalRegistryProvenanceV1>,
    pub publish_authority: FinalRegistryPublishAuthorityV1,
    pub authority_provenance: Option<FinalRegistryProvenanceV1>,
}

/// The future external adapter and fixtures return exactly this observation
/// shape. Calls are explicit; implementing this trait does not grant authority.
pub trait FinalRegistryProviderV1 {
    fn observe(&self, row: &FinalRegistryExpectedRowV1) -> FinalRegistryObservationV1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryVersionStateV1 {
    Missing,
    AlreadyPublishedExact,
    AlreadyPublishedConflict,
    Yanked,
    NameUnavailable,
    Unknown,
}

/// Ordered by fail-closed aggregate precedence, with all findings retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryPreflightResultV1 {
    UnsupportedGeneration,
    Malformed,
    InstrumentFailure,
    Conflict,
    Stale,
    ProviderUnavailable,
    Incomplete,
    CompleteWithResidualAuthorityRisk,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalRegistryNextActionV1 {
    RepairInput,
    ResolveConflict,
    RefreshObservation,
    RestoreProvider,
    AwaitVisibility,
    ObtainPrerequisite,
    ObtainAuthorityEvidence,
    AwaitSeparateAuthorization,
    RetainExactPrerequisite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryPreflightFindingV1 {
    pub result: FinalRegistryPreflightResultV1,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalRegistryPreflightRowV1 {
    pub expected: FinalRegistryExpectedRowV1,
    pub observation: Option<FinalRegistryObservationV1>,
    pub version_state: FinalRegistryVersionStateV1,
    pub findings: Vec<FinalRegistryPreflightFindingV1>,
    pub next_action: FinalRegistryNextActionV1,
}

/// Feasibility receipt; even `Complete` is not release authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowFinalRegistryPreflightV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub observed_context: FinalRegistryContextV1,
    pub current_context: FinalRegistryContextV1,
    pub evaluated_at_unix_seconds: u64,
    pub maximum_age_seconds: u64,
    pub result: FinalRegistryPreflightResultV1,
    pub findings: Vec<FinalRegistryPreflightFindingV1>,
    pub upload_rows: Vec<FinalRegistryPreflightRowV1>,
    pub shared_prerequisites: Vec<FinalRegistryPreflightRowV1>,
    /// Unpaired observations in input order; never selected package authority.
    pub surplus_observations: Vec<FinalRegistryObservationV1>,
}

/// Declaration order is canonical; sequence order is retained, never repaired
/// by sorting malformed input. Consumers must reconcile before trusting fields.
pub fn render_final_registry_preflight_v1(
    receipt: &CargoAllowFinalRegistryPreflightV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}
