//! Canonical shared identity, event-chain, head, and aggregate authority for
//! the exact cargo-allow final release operation (#3940).
//!
//! Provider-specific payloads remain owned by the tag, publication, checkpoint,
//! lease, GitHub Release, and recovery contracts. This module owns only the
//! immutable semantic subject and the append-only cross-transaction envelope.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::release_authorization_v1::RELEASE_AUTHORIZATION_SELECTION;

pub const RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION: u32 = 1;
pub const RELEASE_OPERATION_IDENTITY_SCHEMA_ID: &str = "cargo-allow.release-operation-identity.v1";
pub const RELEASE_OPERATION_EVENT_SCHEMA_ID: &str = "cargo-allow.release-operation-event.v1";
pub const RELEASE_OPERATION_HEAD_SCHEMA_ID: &str = "cargo-allow.release-operation-head.v1";
pub const RELEASE_OPERATION_EVALUATION_SCHEMA_ID: &str =
    "cargo-allow.release-operation-evaluation.v1";
pub const RELEASE_OPERATION_GENESIS_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
pub const RELEASE_OPERATION_REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
pub const RELEASE_OPERATION_PRODUCT: &str = "cargo-allow";
pub const RELEASE_OPERATION_VERSION: &str = "0.2.0";
pub const RELEASE_OPERATION_TAG: &str = "v0.2.0";
pub const RELEASE_OPERATION_CHANNEL: &str = "stable";
pub const RELEASE_OPERATION_ASSET_SELECTION: [(&str, &str); 7] = [
    ("release-manifest", "release-manifest-v2.json"),
    ("release-manifest-checksum", "release-manifest-v2.sha256"),
    (
        "linux-archive",
        "cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz",
    ),
    (
        "linux-archive-checksum",
        "cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz.sha256",
    ),
    (
        "linux-executable-checksum",
        "cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz.executable.sha256",
    ),
    ("linux-package-receipt", "release-binary.receipt.json"),
    ("linux-install-receipt", "release-binary-install.receipt.json"),
];

const CLAIM_BOUNDARY: &str = "Canonical semantic identity, append-only event order, current head, and aggregate state for one exact cargo-allow final-release operation. This authority performs no provider call, credential access, tag mutation, package publication, GitHub Release mutation, recovery action, or live-control change.";
const SECRET_MARKERS: [&str; 8] = [
    "BEGIN PRIVATE KEY",
    "github_pat_",
    "ghp_",
    "AKIA",
    "xoxb-",
    "password=",
    "token=",
    "CARGO_REGISTRY_TOKEN",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationClassV1 {
    CleanFinalPublication,
    IncidentRecovery,
    Containment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationAuthorityKindV1 {
    Clean,
    Recovery,
    Containment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationEventClassV1 {
    OperationSelected,
    AuthorizationSelected,
    LeaseAcquired,
    TagIntentDurable,
    TagObservedExact,
    PackageRowIntentDurable,
    PackageRowObservedExact,
    GitHubDraftObservedExact,
    AssetObservedExact,
    PublicReleaseObservedExact,
    RepositoryReconciled,
    IncidentRecorded,
    RecoverySelected,
    OperationSettled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationSemanticResultV1 {
    Exact,
    Partial,
    Unknown,
    Conflict,
    Stale,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationResponsePostureV1 {
    NotApplicable,
    ResponseKnown,
    ResponseUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationEventSubjectV1 {
    Operation,
    Package(String),
    Asset(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationPackageRowV1 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub package_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationAssetRowV1 {
    pub asset_id: String,
    pub asset_name: String,
    pub asset_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowReleaseOperationIdentityInitV1 {
    pub nonce: String,
    pub operation_class: CargoAllowReleaseOperationClassV1,
    pub authority_kind: CargoAllowReleaseOperationAuthorityKindV1,
    pub repository: String,
    pub product: String,
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    pub freeze_digest: String,
    pub final_evidence_graph_digest: String,
    pub custody_digest: String,
    pub replay_digest: String,
    pub authorization_digest: String,
    pub cargo_lock_digest: String,
    pub topology_digest: String,
    pub support_digest: String,
    pub channel_digest: String,
    pub packages: Vec<CargoAllowReleaseOperationPackageRowV1>,
    pub assets: Vec<CargoAllowReleaseOperationAssetRowV1>,
    pub workflow_digest: String,
    pub action_inventory_digest: String,
    pub live_controls_digest: String,
    pub incident_predecessor_operation_digest: Option<String>,
    pub one_run_scope: bool,
    pub expires_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationIdentityV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_id: String,
    pub nonce: String,
    pub operation_class: CargoAllowReleaseOperationClassV1,
    pub authority_kind: CargoAllowReleaseOperationAuthorityKindV1,
    pub repository: String,
    pub product: String,
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    pub freeze_digest: String,
    pub final_evidence_graph_digest: String,
    pub custody_digest: String,
    pub replay_digest: String,
    pub authorization_digest: String,
    pub cargo_lock_digest: String,
    pub topology_digest: String,
    pub support_digest: String,
    pub channel_digest: String,
    pub package_denominator_digest: String,
    pub packages: Vec<CargoAllowReleaseOperationPackageRowV1>,
    pub asset_denominator_digest: String,
    pub assets: Vec<CargoAllowReleaseOperationAssetRowV1>,
    pub workflow_digest: String,
    pub action_inventory_digest: String,
    pub live_controls_digest: String,
    pub incident_predecessor_operation_digest: Option<String>,
    pub one_run_scope: bool,
    pub expires_at_unix_seconds: u64,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationProducerV1 {
    pub tool: String,
    pub schema: String,
    pub generation: u32,
    pub repository: String,
    pub workflow: String,
    pub workflow_ref: String,
    pub run: String,
    pub attempt: u32,
    pub job: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowReleaseOperationEventInitV1 {
    pub event_class: CargoAllowReleaseOperationEventClassV1,
    pub subject: CargoAllowReleaseOperationEventSubjectV1,
    pub payload_schema_id: String,
    pub payload_digest: String,
    pub producer: CargoAllowReleaseOperationProducerV1,
    pub actor: String,
    pub authority_class: CargoAllowReleaseOperationAuthorityKindV1,
    pub request_boundary: String,
    pub response_posture: CargoAllowReleaseOperationResponsePostureV1,
    pub semantic_result: CargoAllowReleaseOperationSemanticResultV1,
    pub artifact_digest: Option<String>,
    pub observed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationEventV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_identity_digest: String,
    pub sequence: u64,
    pub previous_event_digest: String,
    pub event_digest: String,
    pub event_class: CargoAllowReleaseOperationEventClassV1,
    pub subject: CargoAllowReleaseOperationEventSubjectV1,
    pub payload_schema_id: String,
    pub payload_digest: String,
    pub producer: CargoAllowReleaseOperationProducerV1,
    pub actor: String,
    pub authority_class: CargoAllowReleaseOperationAuthorityKindV1,
    pub request_boundary: String,
    pub response_posture: CargoAllowReleaseOperationResponsePostureV1,
    pub semantic_result: CargoAllowReleaseOperationSemanticResultV1,
    pub artifact_digest: Option<String>,
    pub observed_at_unix_seconds: u64,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoAllowReleaseOperationStateV1 {
    Prepared,
    Authorized,
    HeldPreIrreversible,
    TagObservedPackagesPending,
    PackagePublicationInProgress,
    PackagesPublishedExact,
    GitHubReleaseInProgress,
    PublicReleaseObserved,
    RepositoryReconciliationRequired,
    CompleteClean,
    CompleteWithIncidentLineage,
    RecoveryRequired,
    Conflict,
    Stale,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationHeadV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_identity_digest: String,
    pub sequence: u64,
    pub event_digest: String,
    pub state: CargoAllowReleaseOperationStateV1,
    pub first_irreversible_event_digest: Option<String>,
    pub incident_lineage: bool,
    pub packages_observed_exact: Vec<String>,
    pub assets_observed_exact: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_identity_digest: String,
    pub state: CargoAllowReleaseOperationStateV1,
    pub head: CargoAllowReleaseOperationHeadV1,
    pub missing_packages: Vec<String>,
    pub missing_assets: Vec<String>,
    pub incident_lineage: bool,
    pub findings: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Serialize)]
struct IdentitySeedV1<'a> {
    nonce: &'a str,
    operation_class: CargoAllowReleaseOperationClassV1,
    authority_kind: CargoAllowReleaseOperationAuthorityKindV1,
    repository: &'a str,
    product: &'a str,
    version: &'a str,
    tag: &'a str,
    channel: &'a str,
    github_prerelease: bool,
    freeze_digest: &'a str,
    final_evidence_graph_digest: &'a str,
    custody_digest: &'a str,
    replay_digest: &'a str,
    authorization_digest: &'a str,
    cargo_lock_digest: &'a str,
    topology_digest: &'a str,
    support_digest: &'a str,
    channel_digest: &'a str,
    package_denominator_digest: &'a str,
    packages: &'a [CargoAllowReleaseOperationPackageRowV1],
    asset_denominator_digest: &'a str,
    assets: &'a [CargoAllowReleaseOperationAssetRowV1],
    workflow_digest: &'a str,
    action_inventory_digest: &'a str,
    live_controls_digest: &'a str,
    incident_predecessor_operation_digest: &'a Option<String>,
    one_run_scope: bool,
    expires_at_unix_seconds: u64,
}

#[derive(Serialize)]
struct EventDigestBodyV1<'a> {
    schema_id: &'a str,
    schema_version: u32,
    operation_identity_digest: &'a str,
    sequence: u64,
    previous_event_digest: &'a str,
    event_class: CargoAllowReleaseOperationEventClassV1,
    subject: &'a CargoAllowReleaseOperationEventSubjectV1,
    payload_schema_id: &'a str,
    payload_digest: &'a str,
    producer: &'a CargoAllowReleaseOperationProducerV1,
    actor: &'a str,
    authority_class: CargoAllowReleaseOperationAuthorityKindV1,
    request_boundary: &'a str,
    response_posture: CargoAllowReleaseOperationResponsePostureV1,
    semantic_result: CargoAllowReleaseOperationSemanticResultV1,
    artifact_digest: &'a Option<String>,
    observed_at_unix_seconds: u64,
    claim_boundary: &'a str,
}

fn digest_bytes(bytes: &[u8]) -> String {
    allow_core::sha256_v1_bytes(bytes).replacen("sha256:v1:", "sha256:", 1)
}

fn digest_json<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

fn digest_shape(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn git_sha_shape(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn contains_secret_marker(value: &str) -> bool {
    SECRET_MARKERS.iter().any(|marker| value.contains(marker))
}

fn denominator_digest<T: Serialize>(rows: &[T]) -> Result<String, serde_json::Error> {
    digest_json(rows)
}

fn validate_package_rows(
    rows: &[CargoAllowReleaseOperationPackageRowV1],
) -> Result<(), &'static str> {
    if rows.len() != 10 {
        return Err("release operation identity requires exactly ten final package rows");
    }
    let expected = RELEASE_AUTHORIZATION_SELECTION
        .iter()
        .filter(|(_, _, _, shared)| !*shared);
    for (row, (logical_id, package_name, version, _)) in rows.iter().zip(expected) {
        if row.logical_id != *logical_id
            || row.package_name != *package_name
            || row.package_version != *version
            || !digest_shape(&row.package_digest)
        {
            return Err(
                "release operation package denominator must equal the selected final release order",
            );
        }
    }
    Ok(())
}

fn validate_asset_rows(rows: &[CargoAllowReleaseOperationAssetRowV1]) -> Result<(), &'static str> {
    if rows.len() != RELEASE_OPERATION_ASSET_SELECTION.len() {
        return Err("release operation identity requires the exact selected asset denominator");
    }
    for (row, (asset_id, asset_name)) in rows.iter().zip(RELEASE_OPERATION_ASSET_SELECTION) {
        if row.asset_id != asset_id
            || row.asset_name != asset_name
            || !digest_shape(&row.asset_digest)
        {
            return Err(
                "release operation asset denominator must equal the selected GitHub Release attachment order",
            );
        }
    }
    Ok(())
}

fn validate_authority_lineage(
    operation_class: CargoAllowReleaseOperationClassV1,
    authority_kind: CargoAllowReleaseOperationAuthorityKindV1,
    predecessor: &Option<String>,
) -> Result<(), &'static str> {
    match (operation_class, authority_kind, predecessor) {
        (
            CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            CargoAllowReleaseOperationAuthorityKindV1::Clean,
            None,
        ) => Ok(()),
        (
            CargoAllowReleaseOperationClassV1::IncidentRecovery,
            CargoAllowReleaseOperationAuthorityKindV1::Recovery,
            Some(digest),
        )
        | (
            CargoAllowReleaseOperationClassV1::Containment,
            CargoAllowReleaseOperationAuthorityKindV1::Containment,
            Some(digest),
        ) if digest_shape(digest) => Ok(()),
        _ => Err("operation class, authority kind, and incident predecessor do not agree"),
    }
}

fn identity_seed<'a>(identity: &'a CargoAllowReleaseOperationIdentityV1) -> IdentitySeedV1<'a> {
    IdentitySeedV1 {
        nonce: &identity.nonce,
        operation_class: identity.operation_class,
        authority_kind: identity.authority_kind,
        repository: &identity.repository,
        product: &identity.product,
        version: &identity.version,
        tag: &identity.tag,
        channel: &identity.channel,
        github_prerelease: identity.github_prerelease,
        freeze_digest: &identity.freeze_digest,
        final_evidence_graph_digest: &identity.final_evidence_graph_digest,
        custody_digest: &identity.custody_digest,
        replay_digest: &identity.replay_digest,
        authorization_digest: &identity.authorization_digest,
        cargo_lock_digest: &identity.cargo_lock_digest,
        topology_digest: &identity.topology_digest,
        support_digest: &identity.support_digest,
        channel_digest: &identity.channel_digest,
        package_denominator_digest: &identity.package_denominator_digest,
        packages: &identity.packages,
        asset_denominator_digest: &identity.asset_denominator_digest,
        assets: &identity.assets,
        workflow_digest: &identity.workflow_digest,
        action_inventory_digest: &identity.action_inventory_digest,
        live_controls_digest: &identity.live_controls_digest,
        incident_predecessor_operation_digest: &identity.incident_predecessor_operation_digest,
        one_run_scope: identity.one_run_scope,
        expires_at_unix_seconds: identity.expires_at_unix_seconds,
    }
}

fn derived_operation_id(seed_digest: &str) -> Result<String, &'static str> {
    let hex = seed_digest
        .strip_prefix("sha256:")
        .ok_or("operation identity seed digest is malformed")?;
    let prefix = hex
        .get(..24)
        .ok_or("operation identity seed digest is too short")?;
    Ok(format!("cargo-allow-op-{prefix}"))
}

pub fn build_release_operation_identity_v1(
    init: CargoAllowReleaseOperationIdentityInitV1,
) -> Result<CargoAllowReleaseOperationIdentityV1, &'static str> {
    if init.repository != RELEASE_OPERATION_REPOSITORY
        || init.product != RELEASE_OPERATION_PRODUCT
        || init.version != RELEASE_OPERATION_VERSION
        || init.tag != RELEASE_OPERATION_TAG
        || init.channel != RELEASE_OPERATION_CHANNEL
        || init.github_prerelease
    {
        return Err("operation identity must bind the exact final cargo-allow 0.2.0 subject");
    }
    if init.nonce.trim().is_empty() || contains_secret_marker(&init.nonce) {
        return Err("operation identity requires a bounded non-secret nonce");
    }
    if !init.one_run_scope || init.expires_at_unix_seconds == 0 {
        return Err("operation identity requires one-run scope and a bounded expiry");
    }
    validate_authority_lineage(
        init.operation_class,
        init.authority_kind,
        &init.incident_predecessor_operation_digest,
    )?;
    for digest in [
        init.freeze_digest.as_str(),
        init.final_evidence_graph_digest.as_str(),
        init.custody_digest.as_str(),
        init.replay_digest.as_str(),
        init.authorization_digest.as_str(),
        init.cargo_lock_digest.as_str(),
        init.topology_digest.as_str(),
        init.support_digest.as_str(),
        init.channel_digest.as_str(),
        init.workflow_digest.as_str(),
        init.action_inventory_digest.as_str(),
        init.live_controls_digest.as_str(),
    ] {
        if !digest_shape(digest) {
            return Err("operation identity contains a malformed required digest");
        }
    }
    validate_package_rows(&init.packages)?;
    validate_asset_rows(&init.assets)?;
    let package_denominator_digest =
        denominator_digest(&init.packages).map_err(|_| "package denominator digest failed")?;
    let asset_denominator_digest =
        denominator_digest(&init.assets).map_err(|_| "asset denominator digest failed")?;

    let mut identity = CargoAllowReleaseOperationIdentityV1 {
        schema_id: RELEASE_OPERATION_IDENTITY_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_id: String::new(),
        nonce: init.nonce,
        operation_class: init.operation_class,
        authority_kind: init.authority_kind,
        repository: init.repository,
        product: init.product,
        version: init.version,
        tag: init.tag,
        channel: init.channel,
        github_prerelease: init.github_prerelease,
        freeze_digest: init.freeze_digest,
        final_evidence_graph_digest: init.final_evidence_graph_digest,
        custody_digest: init.custody_digest,
        replay_digest: init.replay_digest,
        authorization_digest: init.authorization_digest,
        cargo_lock_digest: init.cargo_lock_digest,
        topology_digest: init.topology_digest,
        support_digest: init.support_digest,
        channel_digest: init.channel_digest,
        package_denominator_digest,
        packages: init.packages,
        asset_denominator_digest,
        assets: init.assets,
        workflow_digest: init.workflow_digest,
        action_inventory_digest: init.action_inventory_digest,
        live_controls_digest: init.live_controls_digest,
        incident_predecessor_operation_digest: init.incident_predecessor_operation_digest,
        one_run_scope: init.one_run_scope,
        expires_at_unix_seconds: init.expires_at_unix_seconds,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    };
    let seed_digest =
        digest_json(&identity_seed(&identity)).map_err(|_| "operation identity digest failed")?;
    identity.operation_id = derived_operation_id(&seed_digest)?;
    Ok(identity)
}

pub fn validate_release_operation_identity_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
) -> Result<(), &'static str> {
    if identity.schema_id != RELEASE_OPERATION_IDENTITY_SCHEMA_ID
        || identity.schema_version != RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION
        || identity.claim_boundary != CLAIM_BOUNDARY
    {
        return Err("operation identity uses an unsupported schema generation");
    }
    validate_authority_lineage(
        identity.operation_class,
        identity.authority_kind,
        &identity.incident_predecessor_operation_digest,
    )?;
    validate_package_rows(&identity.packages)?;
    validate_asset_rows(&identity.assets)?;
    for digest in [
        identity.freeze_digest.as_str(),
        identity.final_evidence_graph_digest.as_str(),
        identity.custody_digest.as_str(),
        identity.replay_digest.as_str(),
        identity.authorization_digest.as_str(),
        identity.cargo_lock_digest.as_str(),
        identity.topology_digest.as_str(),
        identity.support_digest.as_str(),
        identity.channel_digest.as_str(),
        identity.workflow_digest.as_str(),
        identity.action_inventory_digest.as_str(),
        identity.live_controls_digest.as_str(),
    ] {
        if !digest_shape(digest) {
            return Err("operation identity contains a malformed required digest");
        }
    }
    let package_digest =
        denominator_digest(&identity.packages).map_err(|_| "package denominator digest failed")?;
    let asset_digest =
        denominator_digest(&identity.assets).map_err(|_| "asset denominator digest failed")?;
    if package_digest != identity.package_denominator_digest
        || asset_digest != identity.asset_denominator_digest
    {
        return Err("operation denominator digest does not match its exact rows");
    }
    let seed_digest =
        digest_json(&identity_seed(identity)).map_err(|_| "operation identity digest failed")?;
    if identity.operation_id != derived_operation_id(&seed_digest)? {
        return Err("operation ID does not match the canonical semantic subject");
    }
    if identity.repository != RELEASE_OPERATION_REPOSITORY
        || identity.product != RELEASE_OPERATION_PRODUCT
        || identity.version != RELEASE_OPERATION_VERSION
        || identity.tag != RELEASE_OPERATION_TAG
        || identity.channel != RELEASE_OPERATION_CHANNEL
        || identity.github_prerelease
        || !identity.one_run_scope
        || identity.expires_at_unix_seconds == 0
        || identity.nonce.trim().is_empty()
        || contains_secret_marker(&identity.nonce)
    {
        return Err("operation identity no longer matches the selected final subject");
    }
    Ok(())
}

pub fn release_operation_identity_digest_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
) -> Result<String, serde_json::Error> {
    digest_json(identity)
}

fn validate_producer(producer: &CargoAllowReleaseOperationProducerV1) -> Result<(), &'static str> {
    for value in [
        producer.tool.as_str(),
        producer.schema.as_str(),
        producer.repository.as_str(),
        producer.workflow.as_str(),
        producer.workflow_ref.as_str(),
        producer.run.as_str(),
        producer.job.as_str(),
    ] {
        if value.trim().is_empty() || contains_secret_marker(value) {
            return Err("operation event producer identity is malformed");
        }
    }
    if producer.generation == 0 || producer.attempt == 0 || !git_sha_shape(&producer.commit) {
        return Err("operation event producer generation/attempt/commit is malformed");
    }
    if producer.repository != RELEASE_OPERATION_REPOSITORY {
        return Err("operation event producer belongs to another repository");
    }
    Ok(())
}

fn package_subject_exists(
    identity: &CargoAllowReleaseOperationIdentityV1,
    logical_id: &str,
) -> bool {
    identity
        .packages
        .iter()
        .any(|row| row.logical_id == logical_id)
}

fn asset_subject_exists(identity: &CargoAllowReleaseOperationIdentityV1, asset_id: &str) -> bool {
    identity.assets.iter().any(|row| row.asset_id == asset_id)
}

fn validate_event_subject(
    identity: &CargoAllowReleaseOperationIdentityV1,
    event_class: CargoAllowReleaseOperationEventClassV1,
    subject: &CargoAllowReleaseOperationEventSubjectV1,
) -> Result<(), &'static str> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1 as Subject;
    match (event_class, subject) {
        (Event::PackageRowIntentDurable | Event::PackageRowObservedExact, Subject::Package(id))
            if package_subject_exists(identity, id) =>
        {
            Ok(())
        }
        (Event::AssetObservedExact, Subject::Asset(id)) if asset_subject_exists(identity, id) => {
            Ok(())
        }
        (
            Event::OperationSelected
            | Event::AuthorizationSelected
            | Event::LeaseAcquired
            | Event::TagIntentDurable
            | Event::TagObservedExact
            | Event::GitHubDraftObservedExact
            | Event::PublicReleaseObservedExact
            | Event::RepositoryReconciled
            | Event::IncidentRecorded
            | Event::RecoverySelected
            | Event::OperationSettled,
            Subject::Operation,
        ) => Ok(()),
        _ => Err("operation event subject does not belong to its event class/denominator"),
    }
}

fn event_digest_body<'a>(event: &'a CargoAllowReleaseOperationEventV1) -> EventDigestBodyV1<'a> {
    EventDigestBodyV1 {
        schema_id: &event.schema_id,
        schema_version: event.schema_version,
        operation_identity_digest: &event.operation_identity_digest,
        sequence: event.sequence,
        previous_event_digest: &event.previous_event_digest,
        event_class: event.event_class,
        subject: &event.subject,
        payload_schema_id: &event.payload_schema_id,
        payload_digest: &event.payload_digest,
        producer: &event.producer,
        actor: &event.actor,
        authority_class: event.authority_class,
        request_boundary: &event.request_boundary,
        response_posture: event.response_posture,
        semantic_result: event.semantic_result,
        artifact_digest: &event.artifact_digest,
        observed_at_unix_seconds: event.observed_at_unix_seconds,
        claim_boundary: &event.claim_boundary,
    }
}

pub fn release_operation_event_digest_v1(
    event: &CargoAllowReleaseOperationEventV1,
) -> Result<String, serde_json::Error> {
    digest_json(&event_digest_body(event))
}

fn validate_event_envelope_fields(
    identity: &CargoAllowReleaseOperationIdentityV1,
    event: &CargoAllowReleaseOperationEventV1,
) -> Result<(), &'static str> {
    validate_producer(&event.producer)?;
    if event.payload_schema_id.trim().is_empty()
        || !digest_shape(&event.payload_digest)
        || event.actor.trim().is_empty()
        || contains_secret_marker(&event.actor)
        || event.request_boundary.trim().is_empty()
        || contains_secret_marker(&event.request_boundary)
        || event.observed_at_unix_seconds == 0
        || event
            .artifact_digest
            .as_ref()
            .is_some_and(|digest| !digest_shape(digest))
    {
        return Err("operation event envelope is malformed");
    }
    if event.authority_class != identity.authority_kind {
        return Err("operation event authority class does not match the immutable operation");
    }
    Ok(())
}

fn has_event(
    events: &[CargoAllowReleaseOperationEventV1],
    class: CargoAllowReleaseOperationEventClassV1,
) -> bool {
    events.iter().any(|event| event.event_class == class)
}

fn exact_package_subjects(events: &[CargoAllowReleaseOperationEventV1]) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| {
            event.event_class == CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
                && event.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Exact
        })
        .filter_map(|event| match &event.subject {
            CargoAllowReleaseOperationEventSubjectV1::Package(id) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn exact_asset_subjects(events: &[CargoAllowReleaseOperationEventV1]) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| {
            event.event_class == CargoAllowReleaseOperationEventClassV1::AssetObservedExact
                && event.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Exact
        })
        .filter_map(|event| match &event.subject {
            CargoAllowReleaseOperationEventSubjectV1::Asset(id) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn all_packages_exact(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> bool {
    let exact = exact_package_subjects(events);
    identity
        .packages
        .iter()
        .all(|row| exact.contains(&row.logical_id))
}

fn all_assets_exact(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> bool {
    let exact = exact_asset_subjects(events);
    identity
        .assets
        .iter()
        .all(|row| exact.contains(&row.asset_id))
}

fn validate_event_transition(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    init: &CargoAllowReleaseOperationEventInitV1,
) -> Result<(), &'static str> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;

    if events
        .last()
        .is_some_and(|event| event.event_class == Event::OperationSettled)
    {
        return Err("operation history is terminal after OperationSettled");
    }
    if identity.operation_class == CargoAllowReleaseOperationClassV1::CleanFinalPublication
        && has_event(events, Event::IncidentRecorded)
    {
        return Err(
            "clean operation cannot continue after an incident; recovery needs its own lineage",
        );
    }

    match init.event_class {
        Event::OperationSelected => {
            if !events.is_empty() {
                return Err("OperationSelected must be the first event");
            }
        }
        Event::AuthorizationSelected => {
            if !has_event(events, Event::OperationSelected) {
                return Err("authorization selection requires OperationSelected");
            }
        }
        Event::LeaseAcquired => {
            if !has_event(events, Event::AuthorizationSelected) {
                return Err("lease acquisition requires AuthorizationSelected");
            }
        }
        Event::TagIntentDurable => {
            if !has_event(events, Event::LeaseAcquired) {
                return Err("tag intent requires the durable operation lease");
            }
        }
        Event::TagObservedExact => {
            let ready = if identity.operation_class
                == CargoAllowReleaseOperationClassV1::CleanFinalPublication
            {
                has_event(events, Event::TagIntentDurable)
            } else {
                has_event(events, Event::LeaseAcquired)
            };
            if !ready {
                return Err("exact tag observation is out of order");
            }
        }
        Event::PackageRowIntentDurable => {
            if !has_event(events, Event::TagObservedExact) {
                return Err("package intent requires exact tag observation");
            }
            if events.iter().any(|event| {
                event.event_class == Event::PackageRowIntentDurable && event.subject == init.subject
            }) {
                return Err("package row durable intent is append-once for one operation");
            }
        }
        Event::PackageRowObservedExact => {
            let CargoAllowReleaseOperationEventSubjectV1::Package(id) = &init.subject else {
                return Err("package observation requires a package subject");
            };
            let intent_exists = events.iter().any(|event| {
                event.event_class == Event::PackageRowIntentDurable
                    && event.subject
                        == CargoAllowReleaseOperationEventSubjectV1::Package(id.clone())
            });
            if !intent_exists {
                return Err("package observation requires the same row's durable intent");
            }
            if events.iter().any(|event| {
                event.event_class == Event::PackageRowObservedExact && event.subject == init.subject
            }) {
                return Err("package row exact observation is append-once for one operation");
            }
        }
        Event::GitHubDraftObservedExact => {
            if !all_packages_exact(identity, events) {
                return Err("GitHub draft observation requires every selected package exact");
            }
        }
        Event::AssetObservedExact => {
            if !has_event(events, Event::GitHubDraftObservedExact) {
                return Err("asset observation requires exact GitHub draft observation");
            }
            if events.iter().any(|event| {
                event.event_class == Event::AssetObservedExact && event.subject == init.subject
            }) {
                return Err("asset exact observation is append-once for one operation");
            }
        }
        Event::PublicReleaseObservedExact => {
            if !all_packages_exact(identity, events) || !all_assets_exact(identity, events) {
                return Err("public release observation requires every package and asset exact");
            }
        }
        Event::RepositoryReconciled => {
            if !has_event(events, Event::PublicReleaseObservedExact) {
                return Err("repository reconciliation follows exact public release observation");
            }
        }
        Event::IncidentRecorded => {
            if events.is_empty() {
                return Err("incident recording requires an existing operation");
            }
        }
        Event::RecoverySelected => {
            if identity.operation_class != CargoAllowReleaseOperationClassV1::IncidentRecovery
                || !has_event(events, Event::OperationSelected)
            {
                return Err("RecoverySelected belongs only to an incident-recovery operation");
            }
        }
        Event::OperationSettled => {
            if !has_event(events, Event::RepositoryReconciled)
                || !all_packages_exact(identity, events)
                || !all_assets_exact(identity, events)
                || !has_event(events, Event::PublicReleaseObservedExact)
            {
                return Err("operation settlement requires the complete selected denominator");
            }
        }
    }

    if matches!(
        init.event_class,
        Event::TagObservedExact
            | Event::PackageRowObservedExact
            | Event::GitHubDraftObservedExact
            | Event::AssetObservedExact
            | Event::PublicReleaseObservedExact
            | Event::RepositoryReconciled
            | Event::OperationSettled
    ) && init.semantic_result != ResultClass::Exact
    {
        return Err("exact completion event classes require semantic_result=Exact");
    }
    Ok(())
}

pub fn append_release_operation_event_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    init: CargoAllowReleaseOperationEventInitV1,
) -> Result<CargoAllowReleaseOperationEventV1, &'static str> {
    validate_release_operation_identity_v1(identity)?;
    validate_release_operation_history_v1(identity, events)?;
    validate_event_subject(identity, init.event_class, &init.subject)?;
    validate_event_transition(identity, events, &init)?;
    validate_producer(&init.producer)?;
    let probe = CargoAllowReleaseOperationEventV1 {
        schema_id: RELEASE_OPERATION_EVENT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_identity_digest: release_operation_identity_digest_v1(identity)
            .map_err(|_| "identity digest failed")?,
        sequence: events.len() as u64 + 1,
        previous_event_digest: events.last().map_or_else(
            || RELEASE_OPERATION_GENESIS_DIGEST.to_string(),
            |event| event.event_digest.clone(),
        ),
        event_digest: RELEASE_OPERATION_GENESIS_DIGEST.to_string(),
        event_class: init.event_class,
        subject: init.subject.clone(),
        payload_schema_id: init.payload_schema_id.clone(),
        payload_digest: init.payload_digest.clone(),
        producer: init.producer.clone(),
        actor: init.actor.clone(),
        authority_class: init.authority_class,
        request_boundary: init.request_boundary.clone(),
        response_posture: init.response_posture,
        semantic_result: init.semantic_result,
        artifact_digest: init.artifact_digest.clone(),
        observed_at_unix_seconds: init.observed_at_unix_seconds,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    };
    validate_event_envelope_fields(identity, &probe)?;

    let operation_identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    let sequence = events.len() as u64 + 1;
    let previous_event_digest = events.last().map_or_else(
        || RELEASE_OPERATION_GENESIS_DIGEST.to_string(),
        |event| event.event_digest.clone(),
    );

    let mut event = CargoAllowReleaseOperationEventV1 {
        schema_id: RELEASE_OPERATION_EVENT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_identity_digest,
        sequence,
        previous_event_digest,
        event_digest: String::new(),
        event_class: init.event_class,
        subject: init.subject,
        payload_schema_id: init.payload_schema_id,
        payload_digest: init.payload_digest,
        producer: init.producer,
        actor: init.actor,
        authority_class: init.authority_class,
        request_boundary: init.request_boundary,
        response_posture: init.response_posture,
        semantic_result: init.semantic_result,
        artifact_digest: init.artifact_digest,
        observed_at_unix_seconds: init.observed_at_unix_seconds,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    };
    event.event_digest =
        release_operation_event_digest_v1(&event).map_err(|_| "event digest failed")?;
    Ok(event)
}

pub fn validate_release_operation_history_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> Result<(), &'static str> {
    validate_release_operation_identity_v1(identity)?;
    let operation_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    let mut previous = RELEASE_OPERATION_GENESIS_DIGEST.to_string();
    let mut accepted: Vec<CargoAllowReleaseOperationEventV1> = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let expected_sequence = index as u64 + 1;
        if event.schema_id != RELEASE_OPERATION_EVENT_SCHEMA_ID
            || event.schema_version != RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION
            || event.operation_identity_digest != operation_digest
            || event.sequence != expected_sequence
            || event.previous_event_digest != previous
            || event.claim_boundary != CLAIM_BOUNDARY
            || !digest_shape(&event.payload_digest)
            || !digest_shape(&event.event_digest)
            || event
                .artifact_digest
                .as_ref()
                .is_some_and(|digest| !digest_shape(digest))
        {
            return Err("release operation event chain identity/sequence is invalid");
        }
        validate_event_envelope_fields(identity, event)?;
        validate_event_subject(identity, event.event_class, &event.subject)?;
        let init = CargoAllowReleaseOperationEventInitV1 {
            event_class: event.event_class,
            subject: event.subject.clone(),
            payload_schema_id: event.payload_schema_id.clone(),
            payload_digest: event.payload_digest.clone(),
            producer: event.producer.clone(),
            actor: event.actor.clone(),
            authority_class: event.authority_class,
            request_boundary: event.request_boundary.clone(),
            response_posture: event.response_posture,
            semantic_result: event.semantic_result,
            artifact_digest: event.artifact_digest.clone(),
            observed_at_unix_seconds: event.observed_at_unix_seconds,
        };
        validate_event_transition(identity, &accepted, &init)?;
        let recomputed =
            release_operation_event_digest_v1(event).map_err(|_| "event digest failed")?;
        if recomputed != event.event_digest {
            return Err("release operation event digest does not match its canonical envelope");
        }
        previous = event.event_digest.clone();
        accepted.push(event.clone());
    }
    Ok(())
}

fn response_unknown_is_resolved(
    events: &[CargoAllowReleaseOperationEventV1],
    index: usize,
) -> bool {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    let Some(event) = events.get(index) else {
        return false;
    };
    match (&event.event_class, &event.subject) {
        (Event::TagIntentDurable, CargoAllowReleaseOperationEventSubjectV1::Operation) => {
            events.iter().skip(index + 1).any(|later| {
                later.event_class == Event::TagObservedExact
                    && later.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Exact
            })
        }
        (Event::PackageRowIntentDurable, CargoAllowReleaseOperationEventSubjectV1::Package(id)) => {
            events.iter().skip(index + 1).any(|later| {
                later.event_class == Event::PackageRowObservedExact
                    && later.subject
                        == CargoAllowReleaseOperationEventSubjectV1::Package(id.clone())
                    && later.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Exact
            })
        }
        _ => false,
    }
}

fn limiting_state(
    events: &[CargoAllowReleaseOperationEventV1],
) -> Option<CargoAllowReleaseOperationStateV1> {
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;
    if events
        .iter()
        .any(|event| event.semantic_result == ResultClass::InstrumentFailure)
    {
        return Some(State::InstrumentFailure);
    }
    if events
        .iter()
        .any(|event| event.semantic_result == ResultClass::ProviderUnavailable)
    {
        return Some(State::ProviderUnavailable);
    }
    if events
        .iter()
        .any(|event| event.semantic_result == ResultClass::Conflict)
    {
        return Some(State::Conflict);
    }
    if events
        .iter()
        .any(|event| event.semantic_result == ResultClass::Stale)
    {
        return Some(State::Stale);
    }
    if events
        .iter()
        .any(|event| event.semantic_result == ResultClass::Partial)
    {
        return Some(State::RecoveryRequired);
    }
    if events.iter().enumerate().any(|(index, event)| {
        (event.semantic_result == ResultClass::Unknown
            || event.response_posture
                == CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown)
            && !response_unknown_is_resolved(events, index)
    }) {
        return Some(State::RecoveryRequired);
    }
    None
}

fn evaluate_state(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> CargoAllowReleaseOperationStateV1 {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationStateV1 as State;
    if let Some(state) = limiting_state(events) {
        return state;
    }
    if events
        .iter()
        .any(|event| event.event_class == Event::IncidentRecorded)
    {
        return State::RecoveryRequired;
    }
    if events.is_empty() || !has_event(events, Event::AuthorizationSelected) {
        return State::Prepared;
    }
    if !has_event(events, Event::LeaseAcquired) {
        return State::Authorized;
    }
    if !has_event(events, Event::TagObservedExact) {
        return State::HeldPreIrreversible;
    }
    if !all_packages_exact(identity, events) {
        if has_event(events, Event::PackageRowIntentDurable)
            || !exact_package_subjects(events).is_empty()
        {
            return State::PackagePublicationInProgress;
        }
        return State::TagObservedPackagesPending;
    }
    if !has_event(events, Event::GitHubDraftObservedExact) {
        return State::PackagesPublishedExact;
    }
    if !all_assets_exact(identity, events) || !has_event(events, Event::PublicReleaseObservedExact)
    {
        return State::GitHubReleaseInProgress;
    }
    if !has_event(events, Event::RepositoryReconciled) {
        return State::PublicReleaseObserved;
    }
    if !has_event(events, Event::OperationSettled) {
        return State::RepositoryReconciliationRequired;
    }
    if identity.operation_class == CargoAllowReleaseOperationClassV1::CleanFinalPublication {
        State::CompleteClean
    } else {
        State::CompleteWithIncidentLineage
    }
}

fn first_irreversible_digest(events: &[CargoAllowReleaseOperationEventV1]) -> Option<String> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    events
        .iter()
        .find(|event| {
            matches!(
                event.event_class,
                Event::TagObservedExact
                    | Event::PackageRowObservedExact
                    | Event::GitHubDraftObservedExact
                    | Event::AssetObservedExact
                    | Event::PublicReleaseObservedExact
            )
        })
        .map(|event| event.event_digest.clone())
}

pub fn compile_release_operation_head_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> Result<CargoAllowReleaseOperationHeadV1, &'static str> {
    validate_release_operation_history_v1(identity, events)?;
    let operation_identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    let packages_observed_exact = exact_package_subjects(events).into_iter().collect();
    let assets_observed_exact = exact_asset_subjects(events).into_iter().collect();
    Ok(CargoAllowReleaseOperationHeadV1 {
        schema_id: RELEASE_OPERATION_HEAD_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_identity_digest,
        sequence: events.len() as u64,
        event_digest: events.last().map_or_else(
            || RELEASE_OPERATION_GENESIS_DIGEST.to_string(),
            |event| event.event_digest.clone(),
        ),
        state: evaluate_state(identity, events),
        first_irreversible_event_digest: first_irreversible_digest(events),
        incident_lineage: identity.incident_predecessor_operation_digest.is_some()
            || has_event(
                events,
                CargoAllowReleaseOperationEventClassV1::IncidentRecorded,
            ),
        packages_observed_exact,
        assets_observed_exact,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

pub fn evaluate_release_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> Result<CargoAllowReleaseOperationEvaluationV1, &'static str> {
    let head = compile_release_operation_head_v1(identity, events)?;
    let exact_packages = exact_package_subjects(events);
    let exact_assets = exact_asset_subjects(events);
    let missing_packages = identity
        .packages
        .iter()
        .filter(|row| !exact_packages.contains(&row.logical_id))
        .map(|row| row.logical_id.clone())
        .collect::<Vec<_>>();
    let missing_assets = identity
        .assets
        .iter()
        .filter(|row| !exact_assets.contains(&row.asset_id))
        .map(|row| row.asset_id.clone())
        .collect::<Vec<_>>();
    let mut findings = Vec::new();
    if !missing_packages.is_empty() {
        findings.push("selected package denominator is incomplete".to_string());
    }
    if !missing_assets.is_empty() {
        findings.push("selected asset denominator is incomplete".to_string());
    }
    if head.incident_lineage {
        findings.push("operation retains incident/recovery lineage".to_string());
    }
    Ok(CargoAllowReleaseOperationEvaluationV1 {
        schema_id: RELEASE_OPERATION_EVALUATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_identity_digest: head.operation_identity_digest.clone(),
        state: head.state,
        head,
        missing_packages,
        missing_assets,
        incident_lineage: identity.incident_predecessor_operation_digest.is_some()
            || has_event(
                events,
                CargoAllowReleaseOperationEventClassV1::IncidentRecorded,
            ),
        findings,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

pub fn render_release_operation_identity_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(identity)
}

pub fn render_release_operation_event_v1(
    event: &CargoAllowReleaseOperationEventV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(event)
}

pub fn render_release_operation_head_v1(
    head: &CargoAllowReleaseOperationHeadV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(head)
}

pub fn render_release_operation_evaluation_v1(
    evaluation: &CargoAllowReleaseOperationEvaluationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(evaluation)
}
