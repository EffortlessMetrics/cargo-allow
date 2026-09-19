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
    IrreversibleRequestStarted,
    TagObservedExact,
    PackageRowIntentDurable,
    PackageRowObservedExact,
    GitHubDraftObservedExact,
    AssetObservedExact,
    PublicReleaseObservedExact,
    RepositoryReconciled,
    IncidentRecorded,
    RecoverySelected,
    ContainmentSelected,
    ContainmentObservedExact,
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
    pub incident_predecessor_head_digest: Option<String>,
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
    pub incident_predecessor_head_digest: Option<String>,
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
    pub evaluated_at_unix_seconds: u64,
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
    pub evaluated_at_unix_seconds: u64,
    pub head: CargoAllowReleaseOperationHeadV1,
    pub missing_packages: Vec<String>,
    pub missing_assets: Vec<String>,
    pub incident_lineage: bool,
    pub findings: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowReleaseOperationPredecessorProofV1 {
    successor_operation_identity_digest: String,
    predecessor_operation_identity_digest: String,
    predecessor_head_digest: String,
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
    incident_predecessor_head_digest: &'a Option<String>,
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

fn lowercase_hex_shape(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn digest_shape(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| lowercase_hex_shape(hex, 64))
}

fn git_sha_shape(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && lowercase_hex_shape(value, value.len())
}

fn contains_secret_marker(value: &str) -> bool {
    let lowercase = value.to_ascii_lowercase();
    SECRET_MARKERS
        .iter()
        .any(|marker| lowercase.contains(&marker.to_ascii_lowercase()))
}

fn bounded_retained_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|ch| matches!(ch, '\n' | '\r' | '\0'))
        && !contains_secret_marker(value)
}

fn denominator_digest<T: Serialize>(rows: &[T]) -> Result<String, serde_json::Error> {
    digest_json(rows)
}

fn validate_package_rows(
    rows: &[CargoAllowReleaseOperationPackageRowV1],
) -> Result<(), &'static str> {
    let expected = RELEASE_AUTHORIZATION_SELECTION
        .iter()
        .filter(|(_, _, _, shared)| !*shared)
        .collect::<Vec<_>>();
    if rows.len() != expected.len() {
        return Err("release operation identity requires exactly the selected final package rows");
    }
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
    predecessor_operation: &Option<String>,
    predecessor_head: &Option<String>,
) -> Result<(), &'static str> {
    match (
        operation_class,
        authority_kind,
        predecessor_operation,
        predecessor_head,
    ) {
        (
            CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            CargoAllowReleaseOperationAuthorityKindV1::Clean,
            None,
            None,
        ) => Ok(()),
        (
            CargoAllowReleaseOperationClassV1::IncidentRecovery,
            CargoAllowReleaseOperationAuthorityKindV1::Recovery,
            Some(operation_digest),
            Some(head_digest),
        )
        | (
            CargoAllowReleaseOperationClassV1::Containment,
            CargoAllowReleaseOperationAuthorityKindV1::Containment,
            Some(operation_digest),
            Some(head_digest),
        ) if digest_shape(operation_digest) && digest_shape(head_digest) => Ok(()),
        _ => Err(
            "operation class, authority kind, and incident predecessor binding do not agree",
        ),
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
        incident_predecessor_head_digest: &identity.incident_predecessor_head_digest,
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

fn build_release_operation_identity_unchecked_predecessor_v1(
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
    if !bounded_retained_text(&init.nonce) {
        return Err("operation identity requires a bounded non-secret nonce");
    }
    if !init.one_run_scope || init.expires_at_unix_seconds == 0 {
        return Err("operation identity requires one-run scope and a bounded expiry");
    }
    validate_authority_lineage(
        init.operation_class,
        init.authority_kind,
        &init.incident_predecessor_operation_digest,
        &init.incident_predecessor_head_digest,
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
        incident_predecessor_head_digest: init.incident_predecessor_head_digest,
        one_run_scope: init.one_run_scope,
        expires_at_unix_seconds: init.expires_at_unix_seconds,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    };
    let seed_digest =
        digest_json(&identity_seed(&identity)).map_err(|_| "operation identity digest failed")?;
    identity.operation_id = derived_operation_id(&seed_digest)?;
    Ok(identity)
}

pub fn release_operation_head_digest_v1(
    head: &CargoAllowReleaseOperationHeadV1,
) -> Result<String, serde_json::Error> {
    digest_json(head)
}

fn same_frozen_candidate(
    successor: &CargoAllowReleaseOperationIdentityInitV1,
    predecessor: &CargoAllowReleaseOperationIdentityV1,
) -> bool {
    successor.repository == predecessor.repository
        && successor.product == predecessor.product
        && successor.version == predecessor.version
        && successor.tag == predecessor.tag
        && successor.channel == predecessor.channel
        && successor.github_prerelease == predecessor.github_prerelease
        && successor.freeze_digest == predecessor.freeze_digest
        && successor.final_evidence_graph_digest == predecessor.final_evidence_graph_digest
        && successor.cargo_lock_digest == predecessor.cargo_lock_digest
        && successor.topology_digest == predecessor.topology_digest
        && successor.support_digest == predecessor.support_digest
        && successor.channel_digest == predecessor.channel_digest
        && successor.packages == predecessor.packages
        && successor.assets == predecessor.assets
}

pub fn build_release_operation_identity_v1(
    init: CargoAllowReleaseOperationIdentityInitV1,
) -> Result<CargoAllowReleaseOperationIdentityV1, &'static str> {
    if init.operation_class != CargoAllowReleaseOperationClassV1::CleanFinalPublication {
        return Err("non-clean operation identity requires a validated predecessor");
    }
    build_release_operation_identity_unchecked_predecessor_v1(init)
}

pub fn build_release_operation_identity_with_predecessor_v1(
    mut init: CargoAllowReleaseOperationIdentityInitV1,
    predecessor_identity: &CargoAllowReleaseOperationIdentityV1,
    predecessor_events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationIdentityV1, &'static str> {
    if init.operation_class == CargoAllowReleaseOperationClassV1::CleanFinalPublication {
        return Err("clean operation identity must not carry a predecessor");
    }
    if predecessor_identity.operation_class
        != CargoAllowReleaseOperationClassV1::CleanFinalPublication
    {
        return Err("v1 predecessor must be the original clean operation");
    }
    validate_release_operation_identity_v1(predecessor_identity)?;
    validate_release_operation_history_v1(predecessor_identity, predecessor_events)?;
    if !same_frozen_candidate(&init, predecessor_identity) {
        return Err("non-clean operation must retain the exact frozen candidate");
    }
    let predecessor = evaluate_release_operation_v1(
        predecessor_identity,
        predecessor_events,
        evaluated_at_unix_seconds,
    )?;
    if !predecessor.incident_lineage
        || predecessor.state == CargoAllowReleaseOperationStateV1::CompleteClean
    {
        return Err("non-clean operation requires an incident-bearing predecessor");
    }
    let predecessor_operation_digest =
        release_operation_identity_digest_v1(predecessor_identity)
            .map_err(|_| "predecessor identity digest failed")?;
    let predecessor_head_digest = release_operation_head_digest_v1(&predecessor.head)
        .map_err(|_| "predecessor head digest failed")?;
    if init
        .incident_predecessor_operation_digest
        .as_ref()
        .is_some_and(|digest| digest != &predecessor_operation_digest)
        || init
            .incident_predecessor_head_digest
            .as_ref()
            .is_some_and(|digest| digest != &predecessor_head_digest)
    {
        return Err("caller predecessor binding conflicts with validated predecessor");
    }
    init.incident_predecessor_operation_digest = Some(predecessor_operation_digest);
    init.incident_predecessor_head_digest = Some(predecessor_head_digest);
    build_release_operation_identity_unchecked_predecessor_v1(init)
}

pub fn validate_release_operation_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    predecessor_identity: &CargoAllowReleaseOperationIdentityV1,
    predecessor_events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationPredecessorProofV1, &'static str> {
    if identity.operation_class == CargoAllowReleaseOperationClassV1::CleanFinalPublication {
        return Err("clean operation has no predecessor proof");
    }
    if predecessor_identity.operation_class
        != CargoAllowReleaseOperationClassV1::CleanFinalPublication
    {
        return Err("v1 predecessor proof requires the original clean operation");
    }
    validate_release_operation_identity_v1(identity)?;
    validate_release_operation_identity_v1(predecessor_identity)?;
    validate_release_operation_history_v1(predecessor_identity, predecessor_events)?;
    let predecessor = evaluate_release_operation_v1(
        predecessor_identity,
        predecessor_events,
        evaluated_at_unix_seconds,
    )?;
    if !predecessor.incident_lineage
        || predecessor.state == CargoAllowReleaseOperationStateV1::CompleteClean
    {
        return Err("predecessor proof requires incident-bearing non-clean history");
    }
    let predecessor_operation_identity_digest =
        release_operation_identity_digest_v1(predecessor_identity)
            .map_err(|_| "predecessor identity digest failed")?;
    let predecessor_head_digest = release_operation_head_digest_v1(&predecessor.head)
        .map_err(|_| "predecessor head digest failed")?;
    if identity.incident_predecessor_operation_digest.as_deref()
        != Some(predecessor_operation_identity_digest.as_str())
        || identity.incident_predecessor_head_digest.as_deref()
            != Some(predecessor_head_digest.as_str())
    {
        return Err("non-clean operation is not bound to this exact predecessor head");
    }
    let successor_projection = CargoAllowReleaseOperationIdentityInitV1 {
        nonce: identity.nonce.clone(),
        operation_class: identity.operation_class,
        authority_kind: identity.authority_kind,
        repository: identity.repository.clone(),
        product: identity.product.clone(),
        version: identity.version.clone(),
        tag: identity.tag.clone(),
        channel: identity.channel.clone(),
        github_prerelease: identity.github_prerelease,
        freeze_digest: identity.freeze_digest.clone(),
        final_evidence_graph_digest: identity.final_evidence_graph_digest.clone(),
        custody_digest: identity.custody_digest.clone(),
        replay_digest: identity.replay_digest.clone(),
        authorization_digest: identity.authorization_digest.clone(),
        cargo_lock_digest: identity.cargo_lock_digest.clone(),
        topology_digest: identity.topology_digest.clone(),
        support_digest: identity.support_digest.clone(),
        channel_digest: identity.channel_digest.clone(),
        packages: identity.packages.clone(),
        assets: identity.assets.clone(),
        workflow_digest: identity.workflow_digest.clone(),
        action_inventory_digest: identity.action_inventory_digest.clone(),
        live_controls_digest: identity.live_controls_digest.clone(),
        incident_predecessor_operation_digest: identity
            .incident_predecessor_operation_digest
            .clone(),
        incident_predecessor_head_digest: identity.incident_predecessor_head_digest.clone(),
        one_run_scope: identity.one_run_scope,
        expires_at_unix_seconds: identity.expires_at_unix_seconds,
    };
    if !same_frozen_candidate(&successor_projection, predecessor_identity) {
        return Err("predecessor proof belongs to a different frozen candidate");
    }
    Ok(CargoAllowReleaseOperationPredecessorProofV1 {
        successor_operation_identity_digest: release_operation_identity_digest_v1(identity)
            .map_err(|_| "successor identity digest failed")?,
        predecessor_operation_identity_digest,
        predecessor_head_digest,
    })
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
        &identity.incident_predecessor_head_digest,
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
        || !bounded_retained_text(&identity.nonce)
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

fn validate_predecessor_proof_matches(
    identity: &CargoAllowReleaseOperationIdentityV1,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<(), &'static str> {
    let identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    if identity.operation_class == CargoAllowReleaseOperationClassV1::CleanFinalPublication
        || proof.successor_operation_identity_digest != identity_digest
        || identity.incident_predecessor_operation_digest.as_deref()
            != Some(proof.predecessor_operation_identity_digest.as_str())
        || identity.incident_predecessor_head_digest.as_deref()
            != Some(proof.predecessor_head_digest.as_str())
    {
        return Err("non-clean operation predecessor proof does not match the operation identity");
    }
    Ok(())
}

fn require_clean_operation_api(
    identity: &CargoAllowReleaseOperationIdentityV1,
) -> Result<(), &'static str> {
    if identity.operation_class != CargoAllowReleaseOperationClassV1::CleanFinalPublication {
        return Err("non-clean operation requires the predecessor-bound API");
    }
    Ok(())
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
        if !bounded_retained_text(value) {
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
        (
            Event::PackageRowIntentDurable
            | Event::PackageRowObservedExact
            | Event::IrreversibleRequestStarted,
            Subject::Package(id),
        ) if package_subject_exists(identity, id) =>
        {
            Ok(())
        }
        (
            Event::AssetObservedExact | Event::IrreversibleRequestStarted,
            Subject::Asset(id),
        ) if asset_subject_exists(identity, id) => Ok(()),
        (
            Event::OperationSelected
            | Event::AuthorizationSelected
            | Event::LeaseAcquired
            | Event::TagIntentDurable
            | Event::IrreversibleRequestStarted
            | Event::TagObservedExact
            | Event::GitHubDraftObservedExact
            | Event::PublicReleaseObservedExact
            | Event::RepositoryReconciled
            | Event::IncidentRecorded
            | Event::RecoverySelected
            | Event::ContainmentSelected
            | Event::ContainmentObservedExact
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
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationResponsePostureV1 as Response;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;

    validate_producer(&event.producer)?;
    if event.observed_at_unix_seconds > identity.expires_at_unix_seconds {
        return Err("operation event is outside the operation expiry");
    }
    if !bounded_retained_text(&event.payload_schema_id)
        || !digest_shape(&event.payload_digest)
        || !bounded_retained_text(&event.actor)
        || !bounded_retained_text(&event.request_boundary)
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
    match event.event_class {
        Event::IrreversibleRequestStarted => {
            if event.semantic_result != ResultClass::Unknown
                || event.response_posture != Response::ResponseUnknown
                || event.artifact_digest.is_none()
            {
                return Err(
                    "irreversible request start requires unknown response posture and artifact identity",
                );
            }
        }
        Event::TagObservedExact
        | Event::PackageRowObservedExact
        | Event::GitHubDraftObservedExact
        | Event::AssetObservedExact
        | Event::PublicReleaseObservedExact
        | Event::ContainmentObservedExact => {
            if event.semantic_result != ResultClass::Exact
                || event.response_posture != Response::ResponseKnown
                || event.artifact_digest.is_none()
            {
                return Err("exact provider observation requires known response and artifact identity");
            }
        }
        _ => {
            if event.response_posture != Response::NotApplicable
                || event.semantic_result == ResultClass::Unknown
            {
                return Err("local operation event requires a determinate non-provider posture");
            }
        }
    }
    Ok(())
}

fn has_event(
    events: &[CargoAllowReleaseOperationEventV1],
    class: CargoAllowReleaseOperationEventClassV1,
) -> bool {
    events.iter().any(|event| event.event_class == class)
}

fn event_is_exact_authority(event: &CargoAllowReleaseOperationEventV1) -> bool {
    event.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Exact
        && event.response_posture != CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown
}

fn has_exact_event(
    events: &[CargoAllowReleaseOperationEventV1],
    class: CargoAllowReleaseOperationEventClassV1,
) -> bool {
    events
        .iter()
        .any(|event| event.event_class == class && event_is_exact_authority(event))
}

fn is_singleton_event_class(class: CargoAllowReleaseOperationEventClassV1) -> bool {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    matches!(
        class,
        Event::OperationSelected
            | Event::AuthorizationSelected
            | Event::LeaseAcquired
            | Event::TagIntentDurable
            | Event::TagObservedExact
            | Event::GitHubDraftObservedExact
            | Event::PublicReleaseObservedExact
            | Event::RepositoryReconciled
            | Event::RecoverySelected
            | Event::ContainmentSelected
            | Event::ContainmentObservedExact
            | Event::OperationSettled
    )
}

fn init_resolves_unknown(
    events: &[CargoAllowReleaseOperationEventV1],
    init: &CargoAllowReleaseOperationEventInitV1,
    origin_class: CargoAllowReleaseOperationEventClassV1,
) -> bool {
    events.iter().any(|event| {
        event.event_class == origin_class
            && (event.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Unknown
                || event.response_posture
                    == CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown)
            && init.semantic_result == CargoAllowReleaseOperationSemanticResultV1::Exact
            && init.response_posture
                != CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown
            && event.subject == init.subject
            && event.payload_schema_id == init.payload_schema_id
            && event.payload_digest == init.payload_digest
            && event.request_boundary == init.request_boundary
            && event.artifact_digest.is_some()
            && event.artifact_digest == init.artifact_digest
    })
}

fn exact_package_subjects(events: &[CargoAllowReleaseOperationEventV1]) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| {
            event.event_class == CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
                && event_is_exact_authority(event)
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
                && event_is_exact_authority(event)
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

fn expected_subject_artifact_digest<'a>(
    identity: &'a CargoAllowReleaseOperationIdentityV1,
    subject: &CargoAllowReleaseOperationEventSubjectV1,
) -> Option<&'a str> {
    match subject {
        CargoAllowReleaseOperationEventSubjectV1::Package(id) => identity
            .packages
            .iter()
            .find(|row| row.logical_id == *id)
            .map(|row| row.package_digest.as_str()),
        CargoAllowReleaseOperationEventSubjectV1::Asset(id) => identity
            .assets
            .iter()
            .find(|row| row.asset_id == *id)
            .map(|row| row.asset_digest.as_str()),
        CargoAllowReleaseOperationEventSubjectV1::Operation => None,
    }
}

fn matching_irreversible_request(
    events: &[CargoAllowReleaseOperationEventV1],
    init: &CargoAllowReleaseOperationEventInitV1,
) -> bool {
    events.iter().any(|event| {
        event.event_class == CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted
            && event.subject == init.subject
            && event.payload_schema_id == init.payload_schema_id
            && event.payload_digest == init.payload_digest
            && event.request_boundary == init.request_boundary
            && event.artifact_digest.is_some()
            && event.artifact_digest == init.artifact_digest
    })
}

fn unresolved_irreversible_request_exists(events: &[CargoAllowReleaseOperationEventV1]) -> bool {
    events.iter().enumerate().any(|(index, event)| {
        event.event_class == CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted
            && !response_unknown_is_resolved(events, index)
    })
}

fn validate_event_transition(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    init: &CargoAllowReleaseOperationEventInitV1,
) -> Result<(), &'static str> {
    use CargoAllowReleaseOperationClassV1 as Class;
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1 as Subject;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;

    if events
        .last()
        .is_some_and(|event| event.event_class == Event::OperationSettled)
    {
        return Err("operation history is terminal after OperationSettled");
    }
    if let Some(last) = events.last() {
        if init.observed_at_unix_seconds < last.observed_at_unix_seconds {
            return Err("operation event time must be monotonic");
        }
        let first = &events[0].producer;
        if identity.one_run_scope
            && (init.producer.repository != first.repository
                || init.producer.workflow != first.workflow
                || init.producer.workflow_ref != first.workflow_ref
                || init.producer.run != first.run
                || init.producer.attempt != first.attempt
                || init.producer.commit != first.commit)
        {
            return Err(
                "one-run operation events must retain repository/workflow/ref/run/attempt/commit",
            );
        }
    }
    if is_singleton_event_class(init.event_class) && has_event(events, init.event_class) {
        return Err("singleton operation event was already recorded");
    }
    if events.iter().any(|event| {
        matches!(
            event.semantic_result,
            ResultClass::Partial
                | ResultClass::Conflict
                | ResultClass::Stale
                | ResultClass::ProviderUnavailable
                | ResultClass::InstrumentFailure
        )
    }) && init.event_class != Event::IncidentRecorded
    {
        return Err("non-clean operation result blocks later transitions");
    }
    if unresolved_irreversible_request_exists(events)
        && init.event_class != Event::IncidentRecorded
        && !(matches!(
            init.event_class,
            Event::TagObservedExact
                | Event::PackageRowObservedExact
                | Event::GitHubDraftObservedExact
                | Event::AssetObservedExact
                | Event::PublicReleaseObservedExact
        ) && matching_irreversible_request(events, init))
    {
        return Err("unresolved irreversible response blocks unrelated progression");
    }
    if identity.operation_class == Class::CleanFinalPublication
        && has_event(events, Event::IncidentRecorded)
    {
        return Err(
            "clean operation cannot continue after an incident; recovery needs its own lineage",
        );
    }
    if identity.operation_class == Class::Containment
        && matches!(
            init.event_class,
            Event::TagIntentDurable
                | Event::TagObservedExact
                | Event::PackageRowIntentDurable
                | Event::PackageRowObservedExact
                | Event::GitHubDraftObservedExact
                | Event::AssetObservedExact
                | Event::PublicReleaseObservedExact
        )
    {
        return Err("containment authority cannot create publication progress");
    }

    match init.event_class {
        Event::OperationSelected => {
            if !events.is_empty() || init.semantic_result != ResultClass::Exact {
                return Err("OperationSelected must be the first exact event");
            }
        }
        Event::RecoverySelected => {
            if identity.operation_class != Class::IncidentRecovery
                || !has_exact_event(events, Event::OperationSelected)
                || identity.incident_predecessor_operation_digest.as_deref()
                    != Some(init.payload_digest.as_str())
                || init.semantic_result != ResultClass::Exact
            {
                return Err(
                    "RecoverySelected requires the exact validated predecessor on recovery authority",
                );
            }
        }
        Event::ContainmentSelected => {
            if identity.operation_class != Class::Containment
                || !has_exact_event(events, Event::OperationSelected)
                || identity.incident_predecessor_operation_digest.as_deref()
                    != Some(init.payload_digest.as_str())
                || init.semantic_result != ResultClass::Exact
            {
                return Err(
                    "ContainmentSelected requires the exact validated predecessor on containment authority",
                );
            }
        }
        Event::AuthorizationSelected => {
            let ready = match identity.operation_class {
                Class::CleanFinalPublication => has_exact_event(events, Event::OperationSelected),
                Class::IncidentRecovery => has_exact_event(events, Event::RecoverySelected),
                Class::Containment => has_exact_event(events, Event::ContainmentSelected),
            };
            if !ready {
                return Err("authorization selection requires exact operation-class selection");
            }
        }
        Event::LeaseAcquired => {
            if !has_exact_event(events, Event::AuthorizationSelected) {
                return Err("lease acquisition requires exact AuthorizationSelected");
            }
        }
        Event::TagIntentDurable => {
            if identity.operation_class != Class::CleanFinalPublication
                || !has_exact_event(events, Event::LeaseAcquired)
            {
                return Err("tag intent belongs only to an exact clean leased operation");
            }
        }
        Event::IrreversibleRequestStarted => {
            if let Some(expected) = expected_subject_artifact_digest(identity, &init.subject) {
                if init.artifact_digest.as_deref() != Some(expected) {
                    return Err("provider request bytes do not match the immutable denominator");
                }
            }
            match &init.subject {
                Subject::Operation => {
                    let tag_request = identity.operation_class == Class::CleanFinalPublication
                        && has_exact_event(events, Event::TagIntentDurable)
                        && !has_exact_event(events, Event::TagObservedExact);
                    let draft_request = identity.operation_class != Class::Containment
                        && all_packages_exact(identity, events)
                        && !has_exact_event(events, Event::GitHubDraftObservedExact);
                    let public_request = identity.operation_class != Class::Containment
                        && all_assets_exact(identity, events)
                        && has_exact_event(events, Event::GitHubDraftObservedExact)
                        && !has_exact_event(events, Event::PublicReleaseObservedExact);
                    let containment_request = identity.operation_class == Class::Containment
                        && has_exact_event(events, Event::ContainmentSelected)
                        && has_exact_event(events, Event::AuthorizationSelected)
                        && has_exact_event(events, Event::LeaseAcquired)
                        && !has_exact_event(events, Event::ContainmentObservedExact);
                    if !(tag_request || draft_request || public_request || containment_request) {
                        return Err("operation request start is out of order");
                    }
                }
                Subject::Package(id) => {
                    let subject = Subject::Package(id.clone());
                    if identity.operation_class == Class::Containment
                        || !has_exact_event(events, Event::TagObservedExact)
                        || !events.iter().any(|event| {
                            event.event_class == Event::PackageRowIntentDurable
                                && event.subject == subject
                                && event_is_exact_authority(event)
                        })
                        || events.iter().any(|event| {
                            event.event_class == Event::PackageRowObservedExact
                                && event.subject == subject
                        })
                    {
                        return Err("package request requires one exact durable row intent");
                    }
                }
                Subject::Asset(id) => {
                    let subject = Subject::Asset(id.clone());
                    if identity.operation_class == Class::Containment
                        || !has_exact_event(events, Event::GitHubDraftObservedExact)
                        || events.iter().any(|event| {
                            event.event_class == Event::AssetObservedExact
                                && event.subject == subject
                        })
                    {
                        return Err("asset request requires exact GitHub draft and unobserved asset");
                    }
                }
            }
        }
        Event::TagObservedExact => {
            let ready = match identity.operation_class {
                Class::CleanFinalPublication => {
                    has_exact_event(events, Event::TagIntentDurable)
                        && matching_irreversible_request(events, init)
                }
                Class::IncidentRecovery => has_exact_event(events, Event::LeaseAcquired),
                Class::Containment => false,
            };
            if !ready {
                return Err("exact tag observation is out of order");
            }
        }
        Event::PackageRowIntentDurable => {
            if !has_exact_event(events, Event::TagObservedExact) {
                return Err("package intent requires exact tag observation");
            }
            if events.iter().any(|event| {
                event.event_class == Event::PackageRowIntentDurable && event.subject == init.subject
            }) {
                return Err("package row durable intent is append-once for one operation");
            }
        }
        Event::PackageRowObservedExact => {
            let Subject::Package(id) = &init.subject else {
                return Err("package observation requires a package subject");
            };
            let subject = Subject::Package(id.clone());
            let expected = expected_subject_artifact_digest(identity, &subject)
                .ok_or("package denominator row is missing")?;
            if init.artifact_digest.as_deref() != Some(expected) {
                return Err("package observation bytes differ from the immutable denominator");
            }
            let upload_path = events.iter().any(|event| {
                event.event_class == Event::PackageRowIntentDurable
                    && event.subject == subject
                    && event_is_exact_authority(event)
            }) && matching_irreversible_request(events, init);
            let read_only_recovery = identity.operation_class == Class::IncidentRecovery
                && has_exact_event(events, Event::TagObservedExact)
                && !events.iter().any(|event| {
                    event.event_class == Event::PackageRowIntentDurable
                        && event.subject == subject
                });
            if !(upload_path || read_only_recovery) {
                return Err(
                    "package observation requires its upload request or read-only recovery reconciliation",
                );
            }
            if events.iter().any(|event| {
                event.event_class == Event::PackageRowObservedExact && event.subject == init.subject
            }) {
                return Err("package row exact observation is append-once for one operation");
            }
        }
        Event::GitHubDraftObservedExact => {
            let mutation_path = matching_irreversible_request(events, init);
            let read_only_recovery = identity.operation_class == Class::IncidentRecovery
                && !unresolved_irreversible_request_exists(events);
            if !all_packages_exact(identity, events) || !(mutation_path || read_only_recovery) {
                return Err(
                    "GitHub draft observation requires every package exact plus its request or read-only recovery reconciliation",
                );
            }
        }
        Event::AssetObservedExact => {
            let expected = expected_subject_artifact_digest(identity, &init.subject)
                .ok_or("asset denominator row is missing")?;
            if init.artifact_digest.as_deref() != Some(expected) {
                return Err("asset observation bytes differ from the immutable denominator");
            }
            let mutation_path = matching_irreversible_request(events, init);
            let read_only_recovery = identity.operation_class == Class::IncidentRecovery
                && !unresolved_irreversible_request_exists(events);
            if !has_exact_event(events, Event::GitHubDraftObservedExact)
                || !(mutation_path || read_only_recovery)
            {
                return Err(
                    "asset observation requires exact draft plus its request or read-only recovery reconciliation",
                );
            }
            if events.iter().any(|event| {
                event.event_class == Event::AssetObservedExact && event.subject == init.subject
            }) {
                return Err("asset exact observation is append-once for one operation");
            }
        }
        Event::PublicReleaseObservedExact => {
            let mutation_path = matching_irreversible_request(events, init);
            let read_only_recovery = identity.operation_class == Class::IncidentRecovery
                && !unresolved_irreversible_request_exists(events);
            if !all_packages_exact(identity, events)
                || !all_assets_exact(identity, events)
                || !(mutation_path || read_only_recovery)
            {
                return Err(
                    "public release observation requires the full denominator plus its request or read-only recovery reconciliation",
                );
            }
        }
        Event::ContainmentObservedExact => {
            if identity.operation_class != Class::Containment
                || !matching_irreversible_request(events, init)
            {
                return Err("containment observation requires the exact containment request");
            }
        }
        Event::RepositoryReconciled => {
            let ready = match identity.operation_class {
                Class::Containment => has_exact_event(events, Event::ContainmentObservedExact),
                Class::CleanFinalPublication | Class::IncidentRecovery => {
                    has_exact_event(events, Event::PublicReleaseObservedExact)
                }
            };
            if !ready {
                return Err("repository reconciliation is out of order");
            }
        }
        Event::IncidentRecorded => {
            if events.is_empty() {
                return Err("incident recording requires an existing operation");
            }
        }
        Event::OperationSettled => match identity.operation_class {
            Class::Containment => {
                if init.semantic_result != ResultClass::Exact
                    || !has_exact_event(events, Event::ContainmentSelected)
                    || !has_exact_event(events, Event::AuthorizationSelected)
                    || !has_exact_event(events, Event::LeaseAcquired)
                    || !has_exact_event(events, Event::ContainmentObservedExact)
                    || !has_exact_event(events, Event::RepositoryReconciled)
                {
                    return Err(
                        "containment settlement requires exact action observation and reconciliation",
                    );
                }
            }
            Class::CleanFinalPublication | Class::IncidentRecovery => {
                if init.semantic_result != ResultClass::Exact {
                    return Err("operation settlement requires an exact terminal result");
                }
                if identity.operation_class == Class::IncidentRecovery
                    && !has_exact_event(events, Event::RecoverySelected)
                {
                    return Err("recovery settlement requires exact RecoverySelected");
                }
                if !has_exact_event(events, Event::RepositoryReconciled)
                    || !all_packages_exact(identity, events)
                    || !all_assets_exact(identity, events)
                    || !has_exact_event(events, Event::PublicReleaseObservedExact)
                {
                    return Err("operation settlement requires complete selected denominator");
                }
            }
        },
    }
    Ok(())
}

fn append_release_operation_event_internal_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    init: CargoAllowReleaseOperationEventInitV1,
) -> Result<CargoAllowReleaseOperationEventV1, &'static str> {
    validate_release_operation_identity_v1(identity)?;
    validate_release_operation_history_internal_v1(identity, events)?;
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

fn validate_release_operation_history_internal_v1(
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

pub fn validate_release_operation_history_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
) -> Result<(), &'static str> {
    require_clean_operation_api(identity)?;
    validate_release_operation_history_internal_v1(identity, events)
}

pub fn validate_release_operation_history_with_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<(), &'static str> {
    validate_predecessor_proof_matches(identity, proof)?;
    validate_release_operation_history_internal_v1(identity, events)
}

pub fn append_release_operation_event_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    init: CargoAllowReleaseOperationEventInitV1,
) -> Result<CargoAllowReleaseOperationEventV1, &'static str> {
    require_clean_operation_api(identity)?;
    append_release_operation_event_internal_v1(identity, events, init)
}

pub fn append_release_operation_event_with_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    init: CargoAllowReleaseOperationEventInitV1,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<CargoAllowReleaseOperationEventV1, &'static str> {
    validate_predecessor_proof_matches(identity, proof)?;
    append_release_operation_event_internal_v1(identity, events, init)
}

fn response_unknown_is_resolved(
    events: &[CargoAllowReleaseOperationEventV1],
    index: usize,
) -> bool {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    let Some(event) = events.get(index) else {
        return false;
    };
    if event.event_class != Event::IrreversibleRequestStarted {
        return false;
    }
    events.iter().skip(index + 1).any(|later| {
        let compatible_class = match &event.subject {
            CargoAllowReleaseOperationEventSubjectV1::Operation => matches!(
                later.event_class,
                Event::TagObservedExact
                    | Event::GitHubDraftObservedExact
                    | Event::PublicReleaseObservedExact
                    | Event::ContainmentObservedExact
            ),
            CargoAllowReleaseOperationEventSubjectV1::Package(_) => {
                later.event_class == Event::PackageRowObservedExact
            }
            CargoAllowReleaseOperationEventSubjectV1::Asset(_) => {
                later.event_class == Event::AssetObservedExact
            }
        };
        compatible_class
            && event_is_exact_authority(later)
            && later.subject == event.subject
            && later.payload_schema_id == event.payload_schema_id
            && later.payload_digest == event.payload_digest
            && later.request_boundary == event.request_boundary
            && later.artifact_digest.is_some()
            && later.artifact_digest == event.artifact_digest
    })
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
    evaluated_at_unix_seconds: u64,
) -> CargoAllowReleaseOperationStateV1 {
    use CargoAllowReleaseOperationClassV1 as Class;
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationStateV1 as State;

    if evaluated_at_unix_seconds > identity.expires_at_unix_seconds {
        return State::Stale;
    }
    if let Some(state) = limiting_state(events) {
        return state;
    }
    if identity.operation_class == Class::Containment {
        if has_exact_event(events, Event::OperationSettled) {
            return State::CompleteWithIncidentLineage;
        }
        if has_exact_event(events, Event::ContainmentObservedExact) {
            return State::RepositoryReconciliationRequired;
        }
        if has_exact_event(events, Event::LeaseAcquired) {
            return State::HeldPreIrreversible;
        }
        if has_exact_event(events, Event::AuthorizationSelected) {
            return State::Authorized;
        }
        return State::Prepared;
    }
    if events.iter().any(|event| event.event_class == Event::IncidentRecorded) {
        return State::RecoveryRequired;
    }
    if events.is_empty()
        || (identity.operation_class == Class::IncidentRecovery
            && !has_exact_event(events, Event::RecoverySelected))
        || !has_exact_event(events, Event::AuthorizationSelected)
    {
        return State::Prepared;
    }
    if !has_exact_event(events, Event::LeaseAcquired) {
        return State::Authorized;
    }
    if !has_exact_event(events, Event::TagObservedExact) {
        return State::HeldPreIrreversible;
    }
    if !all_packages_exact(identity, events) {
        if has_event(events, Event::PackageRowIntentDurable) || !exact_package_subjects(events).is_empty() {
            return State::PackagePublicationInProgress;
        }
        return State::TagObservedPackagesPending;
    }
    if !has_exact_event(events, Event::GitHubDraftObservedExact) {
        return State::PackagesPublishedExact;
    }
    if !all_assets_exact(identity, events) || !has_exact_event(events, Event::PublicReleaseObservedExact) {
        return State::GitHubReleaseInProgress;
    }
    if !has_exact_event(events, Event::RepositoryReconciled) {
        return State::PublicReleaseObserved;
    }
    if !has_exact_event(events, Event::OperationSettled) {
        return State::RepositoryReconciliationRequired;
    }
    if identity.operation_class == Class::CleanFinalPublication {
        State::CompleteClean
    } else {
        State::CompleteWithIncidentLineage
    }
}

fn first_irreversible_digest(events: &[CargoAllowReleaseOperationEventV1]) -> Option<String> {
    events
        .iter()
        .find(|event| event.event_class == CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted)
        .map(|event| event.event_digest.clone())
}

fn compile_release_operation_head_internal_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationHeadV1, &'static str> {
    validate_release_operation_history_internal_v1(identity, events)?;
    if evaluated_at_unix_seconds == 0
        || events.last().is_some_and(|event| evaluated_at_unix_seconds < event.observed_at_unix_seconds)
    {
        return Err("release operation head evaluation time is invalid for retained history");
    }
    let operation_identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    let packages_observed_exact = exact_package_subjects(events).into_iter().collect();
    let assets_observed_exact = exact_asset_subjects(events).into_iter().collect();
    Ok(CargoAllowReleaseOperationHeadV1 {
        schema_id: RELEASE_OPERATION_HEAD_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_identity_digest,
        evaluated_at_unix_seconds,
        sequence: events.len() as u64,
        event_digest: events.last().map_or_else(
            || RELEASE_OPERATION_GENESIS_DIGEST.to_string(),
            |event| event.event_digest.clone(),
        ),
        state: evaluate_state(identity, events, evaluated_at_unix_seconds),
        first_irreversible_event_digest: first_irreversible_digest(events),
        incident_lineage: identity.incident_predecessor_operation_digest.is_some()
            || has_event(events, CargoAllowReleaseOperationEventClassV1::IncidentRecorded),
        packages_observed_exact,
        assets_observed_exact,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

fn evaluate_release_operation_internal_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationEvaluationV1, &'static str> {
    if evaluated_at_unix_seconds == 0
        || events.last().is_some_and(|event| {
            evaluated_at_unix_seconds < event.observed_at_unix_seconds
        })
    {
        return Err("release operation evaluation time is invalid for retained history");
    }
    let head = compile_release_operation_head_internal_v1(identity, events, evaluated_at_unix_seconds)?;
    let expired = evaluated_at_unix_seconds > identity.expires_at_unix_seconds;
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
    if expired {
        findings.push("operation identity expired before evaluation".to_string());
    }
    Ok(CargoAllowReleaseOperationEvaluationV1 {
        schema_id: RELEASE_OPERATION_EVALUATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_OPERATION_AUTHORITY_SCHEMA_VERSION,
        operation_identity_digest: head.operation_identity_digest.clone(),
        state: head.state,
        evaluated_at_unix_seconds,
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

pub fn compile_release_operation_head_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationHeadV1, &'static str> {
    require_clean_operation_api(identity)?;
    compile_release_operation_head_internal_v1(identity, events, evaluated_at_unix_seconds)
}

pub fn compile_release_operation_head_with_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<CargoAllowReleaseOperationHeadV1, &'static str> {
    validate_predecessor_proof_matches(identity, proof)?;
    compile_release_operation_head_internal_v1(identity, events, evaluated_at_unix_seconds)
}

pub fn evaluate_release_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationEvaluationV1, &'static str> {
    require_clean_operation_api(identity)?;
    evaluate_release_operation_internal_v1(identity, events, evaluated_at_unix_seconds)
}

pub fn evaluate_release_operation_with_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<CargoAllowReleaseOperationEvaluationV1, &'static str> {
    validate_predecessor_proof_matches(identity, proof)?;
    evaluate_release_operation_internal_v1(identity, events, evaluated_at_unix_seconds)
}

pub fn validate_release_operation_head_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
    head: &CargoAllowReleaseOperationHeadV1,
) -> Result<(), &'static str> {
    let expected = compile_release_operation_head_v1(identity, events, evaluated_at_unix_seconds)?;
    if &expected != head {
        return Err("release operation head does not match the recomputed canonical head");
    }
    Ok(())
}

pub fn validate_release_operation_evaluation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
    evaluation: &CargoAllowReleaseOperationEvaluationV1,
) -> Result<(), &'static str> {
    let expected = evaluate_release_operation_v1(identity, events, evaluated_at_unix_seconds)?;
    if &expected != evaluation {
        return Err("release operation evaluation does not match the recomputed canonical result");
    }
    Ok(())
}

pub fn validate_release_operation_head_with_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
    head: &CargoAllowReleaseOperationHeadV1,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<(), &'static str> {
    let expected = compile_release_operation_head_with_predecessor_v1(
        identity,
        events,
        evaluated_at_unix_seconds,
        proof,
    )?;
    if &expected != head {
        return Err("release operation head does not match the predecessor-bound canonical head");
    }
    Ok(())
}

pub fn validate_release_operation_evaluation_with_predecessor_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    evaluated_at_unix_seconds: u64,
    evaluation: &CargoAllowReleaseOperationEvaluationV1,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
) -> Result<(), &'static str> {
    let expected = evaluate_release_operation_with_predecessor_v1(
        identity,
        events,
        evaluated_at_unix_seconds,
        proof,
    )?;
    if &expected != evaluation {
        return Err(
            "release operation evaluation does not match the predecessor-bound canonical result",
        );
    }
    Ok(())
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
