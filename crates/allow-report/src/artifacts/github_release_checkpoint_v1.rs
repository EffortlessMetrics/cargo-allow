//! Remotely durable checkpoints for the GitHub Release journal (#3933).
//!
//! The journal survives process crashes on one runner; it does not survive
//! total hosted-runner loss. Before each GitHub Release mutation (draft
//! creation, asset upload, finalization) and before a later mutation
//! consumes the resulting state, the operation needs one independently
//! readable remote checkpoint bound to the exact operation, journal prefix,
//! and release identity, so a fresh runner reconstructs the latest honest
//! state instead of re-inferring it.
//!
//! Checkpoints never invent operation identity: every record carries the
//! canonical #3940 `operation_identity_digest` derived from a revalidated
//! canonical identity. Remote checkpoints are independently read back before
//! beginning a mutation and before a later mutation consumes the resulting
//! state. Same-name wrong-byte assets are incidents recorded in the journal,
//! never repaired here.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! release creation, mutation, or deletion, no credential reads, and no
//! live-state mutation. Provider outcomes are caller-supplied; the checkpoint
//! classifies them but never fetches them. A fresh checkpoint reads back as
//! `Missing` until an independent readback is recorded: provider success
//! without readback is never clean.

use serde::{Deserialize, Serialize};

use super::github_release_journal_v1::{
    CargoAllowGitHubReleaseJournalV1, verify_github_release_journal_v1,
};
use super::release_operation_authority_v1::{
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationIdentityV1,
    release_operation_identity_digest_v1, validate_release_operation_identity_v1,
};

pub const GITHUB_RELEASE_CHECKPOINT_SCHEMA_ID: &str = "cargo-allow.github-release-checkpoint.v1";
pub const GITHUB_RELEASE_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

/// Checkpoint operation class, mirroring the journal vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubReleaseCheckpointClassV1 {
    CleanFinalPublication,
    IncidentRecovery,
}

/// Checkpoint position in the mutation lifecycle. Pre-mutation checkpoints
/// gate the API call; post-observation checkpoints gate the next mutation
/// that consumes the resulting state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubReleaseCheckpointKindV1 {
    PreMutationDurable,
    PostObservation,
}

/// Durable provider substrate. Exactly one variant: the selected GitHub
/// Actions Artifact surface. New substrates require a new schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubReleaseCheckpointProviderV1 {
    GithubActionsArtifact,
}

/// Independent readback outcome. Only `Complete` authorizes progress.
/// `ProviderUnavailable` is an outage, never absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubReleaseCheckpointReadbackV1 {
    Complete,
    Missing,
    Stale,
    Mismatch,
    ProviderUnavailable,
    InstrumentFailure,
}

/// The remote provider object. `object_id` is the exact immutable provider
/// identity; `object_name` is a human label only and never participates in
/// discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubReleaseCheckpointProviderObjectV1 {
    pub provider: GitHubReleaseCheckpointProviderV1,
    pub object_id: String,
    pub object_name: String,
    pub object_digest: String,
    pub object_size_bytes: u64,
}

/// The producer identity the release lane expects. Equality with the
/// expected producer is the trust check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubReleaseCheckpointProducerV1 {
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
    pub git_ref: String,
    pub commit: String,
}

/// One remotely durable GitHub Release checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowGitHubReleaseCheckpointV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub checkpoint_id: String,
    pub operation_id: String,
    /// Canonical #3940 operation identity digest. Checkpoints never invent
    /// operation identity: this must equal the digest of the canonical
    /// release-operation identity the checkpoint is bound to.
    pub operation_identity_digest: String,
    pub operation_class: GitHubReleaseCheckpointClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub repository: String,
    pub tag: String,
    pub journal_head_sequence: u64,
    pub journal_head_digest: String,
    pub checkpoint_sequence: u64,
    /// Monotonic linkage: sequence 1 carries `None`; every later checkpoint
    /// carries the canonical digest of its predecessor.
    pub prior_checkpoint_digest: Option<String>,
    pub kind: GitHubReleaseCheckpointKindV1,
    /// GitHub Release ID observed at checkpoint time, when known.
    pub github_release_id: Option<String>,
    /// The mutation this checkpoint gates, as a bounded label (draft-create,
    /// asset-upload:<name>, finalize). Labels route; they never authorize.
    pub gated_mutation: String,
    pub provider: GitHubReleaseCheckpointProviderObjectV1,
    pub producer: GitHubReleaseCheckpointProducerV1,
    pub retention_days: u32,
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub note: String,
    /// Fresh checkpoints start `Missing`: provider success without an
    /// independent readback is never clean.
    pub readback: GitHubReleaseCheckpointReadbackV1,
    pub readback_at_unix_seconds: Option<u64>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// Caller-supplied construction inputs. Linkage and readback are assigned by
/// the checkpoint, never the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubReleaseCheckpointInitV1 {
    pub checkpoint_id: String,
    pub operation_id: String,
    pub operation_identity_digest: String,
    pub operation_class: GitHubReleaseCheckpointClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub repository: String,
    pub tag: String,
    pub journal_head_sequence: u64,
    pub journal_head_digest: String,
    pub checkpoint_sequence: u64,
    pub kind: GitHubReleaseCheckpointKindV1,
    pub github_release_id: Option<String>,
    pub gated_mutation: String,
    pub provider: GitHubReleaseCheckpointProviderObjectV1,
    pub producer: GitHubReleaseCheckpointProducerV1,
    pub retention_days: u32,
    pub created_at_unix_seconds: u64,
    pub note: String,
}

/// Caller-supplied provider outcome. Bytes are downloaded by the caller; the
/// checkpoint classifies them but never fetches them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubCheckpointProviderOutcomeV1 {
    Delivered(Vec<u8>),
    Unavailable,
    InstrumentFailure,
}

/// Runtime-only proof that exact immutable provider bytes were independently
/// read back. Private fields, deliberately not serializable: editing a
/// checkpoint JSON document can never manufacture progress authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubReleaseCheckpointReadbackWitnessV1 {
    checkpoint_link_digest: String,
    provider_object_id: String,
    downloaded_bytes_digest: String,
    observed_at_unix_seconds: u64,
}

pub const GITHUB_RELEASE_CHECKPOINT_MIN_RETENTION_DAYS: u32 = 1;
pub const GITHUB_RELEASE_CHECKPOINT_MAX_RETENTION_DAYS: u32 = 90;
pub const GITHUB_RELEASE_CHECKPOINT_MAX_NOTE_LEN: usize = 256;
const RETENTION_SECONDS_PER_DAY: u64 = 86_400;

const CLAIM_BOUNDARY: &str = "This record owns one remotely durable, independently readable checkpoint per GitHub Release journal prefix: exact operation and journal-prefix identity, monotonic sequence linkage, immutable provider object identity, producer trust, retention, and readback classification. It does not create, edit, publish, or delete a real GitHub Release, authorize the operation, or prove asset semantics independently.";

fn lower_hex_shape(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn digest_shape(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && lower_hex_shape(hex))
}

fn checkpoint_digest_bytes(bytes: &[u8]) -> String {
    allow_core::sha256_v1_bytes(bytes).replacen("sha256:v1:", "sha256:", 1)
}

fn secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "github_pat_",
        "ghp_",
        "password=",
        "token=",
        "cargo_registry_token",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// The checkpoint body: every record field except the provider object and
/// the readback outcome, including both canonical operation digests so a
/// same-name wrong-operation record can never share a body digest.
#[derive(Serialize)]
struct GitHubReleaseCheckpointBodyV1<'a> {
    schema_id: &'a str,
    schema_version: u32,
    checkpoint_id: &'a str,
    operation_id: &'a str,
    operation_identity_digest: &'a str,
    operation_class: GitHubReleaseCheckpointClassV1,
    authorization_digest: &'a str,
    custody_digest: &'a str,
    freeze_digest: &'a str,
    repository: &'a str,
    tag: &'a str,
    journal_head_sequence: u64,
    journal_head_digest: &'a str,
    checkpoint_sequence: u64,
    prior_checkpoint_digest: Option<&'a str>,
    kind: GitHubReleaseCheckpointKindV1,
    github_release_id: Option<&'a str>,
    gated_mutation: &'a str,
    producer: &'a GitHubReleaseCheckpointProducerV1,
    retention_days: u32,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    note: &'a str,
    claim_boundary: &'a str,
    limitations: &'a [String],
}

/// Canonical digest of the checkpoint body, as retained in provider objects.
pub fn digest_github_release_checkpoint_body_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
) -> Result<String, serde_json::Error> {
    let body = GitHubReleaseCheckpointBodyV1 {
        schema_id: &checkpoint.schema_id,
        schema_version: checkpoint.schema_version,
        checkpoint_id: &checkpoint.checkpoint_id,
        operation_id: &checkpoint.operation_id,
        operation_identity_digest: &checkpoint.operation_identity_digest,
        operation_class: checkpoint.operation_class,
        authorization_digest: &checkpoint.authorization_digest,
        custody_digest: &checkpoint.custody_digest,
        freeze_digest: &checkpoint.freeze_digest,
        repository: &checkpoint.repository,
        tag: &checkpoint.tag,
        journal_head_sequence: checkpoint.journal_head_sequence,
        journal_head_digest: &checkpoint.journal_head_digest,
        checkpoint_sequence: checkpoint.checkpoint_sequence,
        prior_checkpoint_digest: checkpoint.prior_checkpoint_digest.as_deref(),
        kind: checkpoint.kind,
        github_release_id: checkpoint.github_release_id.as_deref(),
        gated_mutation: &checkpoint.gated_mutation,
        producer: &checkpoint.producer,
        retention_days: checkpoint.retention_days,
        created_at_unix_seconds: checkpoint.created_at_unix_seconds,
        expires_at_unix_seconds: checkpoint.expires_at_unix_seconds,
        note: &checkpoint.note,
        claim_boundary: &checkpoint.claim_boundary,
        limitations: &checkpoint.limitations,
    };
    let rendered = serde_json::to_vec(&body)?;
    Ok(checkpoint_digest_bytes(&rendered))
}

/// Canonical JSON renderer for GitHub Release checkpoints.
pub fn render_github_release_checkpoint_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(checkpoint)
}

fn canonical_stored_checkpoint_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
) -> Result<CargoAllowGitHubReleaseCheckpointV1, serde_json::Error> {
    let mut stored = checkpoint.clone();
    stored.readback = GitHubReleaseCheckpointReadbackV1::Missing;
    stored.readback_at_unix_seconds = None;
    Ok(stored)
}

fn canonical_stored_checkpoint_bytes_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
) -> Result<Vec<u8>, serde_json::Error> {
    Ok(
        render_github_release_checkpoint_v1(&canonical_stored_checkpoint_v1(checkpoint)?)?
            .into_bytes(),
    )
}

/// Canonical digest of a checkpoint record, binding one sequence link to the
/// stored bytes the provider must return verbatim.
pub fn digest_github_release_checkpoint_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
) -> Result<String, serde_json::Error> {
    let stored = canonical_stored_checkpoint_bytes_v1(checkpoint)?;
    Ok(digest_github_release_checkpoint_bytes_v1(&stored))
}

/// Canonical digest of stored checkpoint bytes, as retained in provider
/// objects for exact-byte readback comparison.
pub fn digest_github_release_checkpoint_bytes_v1(bytes: &[u8]) -> String {
    checkpoint_digest_bytes(bytes)
}

/// Stable predecessor-link digest.
pub fn digest_github_release_checkpoint_link_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
) -> Result<String, serde_json::Error> {
    let stored = canonical_stored_checkpoint_bytes_v1(checkpoint)?;
    Ok(digest_github_release_checkpoint_bytes_v1(&stored))
}

fn validate_producer(producer: &GitHubReleaseCheckpointProducerV1) -> Result<(), &'static str> {
    for value in [
        producer.workflow.as_str(),
        producer.run.as_str(),
        producer.attempt.as_str(),
        producer.job.as_str(),
        producer.git_ref.as_str(),
    ] {
        if value.trim().is_empty() {
            return Err("checkpoint producer identity must be complete");
        }
    }
    if secret_marker(&producer.run) || secret_marker(&producer.job) {
        return Err("secret material must never enter checkpoint records");
    }
    if !(producer.commit.len() == 40 || producer.commit.len() == 64)
        || !lower_hex_shape(&producer.commit)
    {
        return Err("checkpoint producer commit must be a canonical Git SHA");
    }
    Ok(())
}

/// Begin a checkpoint bound to one operation and journal prefix. `prior`
/// carries the predecessor in the same operation when `checkpoint_sequence`
/// exceeds one; linkage is enforced here so a later clean record can never
/// overwrite incident history.
pub fn begin_github_release_checkpoint_v1(
    init: GitHubReleaseCheckpointInitV1,
    prior: Option<&CargoAllowGitHubReleaseCheckpointV1>,
) -> Result<CargoAllowGitHubReleaseCheckpointV1, &'static str> {
    if init.checkpoint_id.trim().is_empty() {
        return Err("checkpoints require checkpoint identity");
    }
    if init.operation_id.trim().is_empty() {
        return Err("checkpoints require an operation name");
    }
    for value in [
        init.operation_identity_digest.as_str(),
        init.authorization_digest.as_str(),
        init.custody_digest.as_str(),
        init.freeze_digest.as_str(),
        init.journal_head_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("checkpoints require canonical operation and journal digests");
        }
    }
    if init.repository.trim().is_empty() || init.tag.trim().is_empty() {
        return Err("checkpoints require repository and tag identity");
    }
    if init.journal_head_sequence < 1 {
        return Err("checkpoints bind a non-empty journal prefix");
    }
    if init.checkpoint_sequence < 1 {
        return Err("checkpoint sequence starts at one");
    }
    if init.gated_mutation.trim().is_empty() {
        return Err("checkpoints name the gated mutation");
    }
    if init
        .github_release_id
        .as_deref()
        .is_some_and(|release_id| release_id.trim().is_empty())
    {
        return Err("checkpoint release IDs must be non-empty when present");
    }
    if init.provider.object_id.trim().is_empty() || init.provider.object_name.trim().is_empty() {
        return Err("checkpoints require exact provider object identity");
    }
    if !digest_shape(&init.provider.object_digest) {
        return Err("checkpoints require a canonical provider object digest");
    }
    if init.provider.object_size_bytes < 1 {
        return Err("checkpoints require a non-empty provider object");
    }
    validate_producer(&init.producer)?;
    if init.retention_days < GITHUB_RELEASE_CHECKPOINT_MIN_RETENTION_DAYS
        || init.retention_days > GITHUB_RELEASE_CHECKPOINT_MAX_RETENTION_DAYS
    {
        return Err("checkpoint retention must fit the provider window");
    }
    if init.created_at_unix_seconds < 1 {
        return Err("checkpoints require a positive construction time");
    }
    let retention_seconds = u64::from(init.retention_days)
        .checked_mul(RETENTION_SECONDS_PER_DAY)
        .ok_or("checkpoint retention overflowed")?;
    let expected_expiry = init
        .created_at_unix_seconds
        .checked_add(retention_seconds)
        .ok_or("checkpoint expiry overflowed")?;
    if init.note.len() > GITHUB_RELEASE_CHECKPOINT_MAX_NOTE_LEN {
        return Err("checkpoint notes are bounded and carry no bodies");
    }
    if secret_marker(&init.note) {
        return Err("secret material must never enter checkpoint records");
    }
    // Monotonic linkage within one operation.
    let prior_checkpoint_digest = match prior {
        None => {
            if init.checkpoint_sequence != 1 {
                return Err("the first checkpoint of an operation starts at sequence one");
            }
            None
        }
        Some(previous) => {
            if init.checkpoint_sequence != previous.checkpoint_sequence + 1 {
                return Err("checkpoint sequence must advance by exactly one");
            }
            for (current, bound) in [
                (init.operation_id.as_str(), previous.operation_id.as_str()),
                (
                    init.operation_identity_digest.as_str(),
                    previous.operation_identity_digest.as_str(),
                ),
                (
                    init.authorization_digest.as_str(),
                    previous.authorization_digest.as_str(),
                ),
                (
                    init.custody_digest.as_str(),
                    previous.custody_digest.as_str(),
                ),
                (init.freeze_digest.as_str(), previous.freeze_digest.as_str()),
                (init.repository.as_str(), previous.repository.as_str()),
                (init.tag.as_str(), previous.tag.as_str()),
            ] {
                if current != bound {
                    return Err("checkpoint linkage never crosses operations");
                }
            }
            if init.operation_class != previous.operation_class {
                return Err("checkpoint linkage never crosses operation classes");
            }
            if previous.readback != GitHubReleaseCheckpointReadbackV1::Complete {
                return Err("checkpoint linkage requires a verified predecessor readback");
            }
            let previous_readback_at = previous
                .readback_at_unix_seconds
                .ok_or("verified predecessor readback requires its observation time")?;
            if init.created_at_unix_seconds < previous.created_at_unix_seconds
                || init.created_at_unix_seconds < previous_readback_at
            {
                return Err("checkpoint construction time never moves backward");
            }
            if init.checkpoint_id == previous.checkpoint_id
                || init.provider.object_id == previous.provider.object_id
            {
                return Err(
                    "later checkpoints require new checkpoint and provider object identities",
                );
            }
            if init.journal_head_sequence < previous.journal_head_sequence {
                return Err("checkpoint journal prefixes never move backward");
            }
            if init.journal_head_sequence == previous.journal_head_sequence
                && init.journal_head_digest != previous.journal_head_digest
            {
                return Err("checkpoint journal prefixes are never rewritten");
            }
            Some(
                digest_github_release_checkpoint_link_v1(previous)
                    .map_err(|_| "checkpoint linkage digest failed")?,
            )
        }
    };
    Ok(CargoAllowGitHubReleaseCheckpointV1 {
        schema_id: GITHUB_RELEASE_CHECKPOINT_SCHEMA_ID.to_string(),
        schema_version: GITHUB_RELEASE_CHECKPOINT_SCHEMA_VERSION,
        checkpoint_id: init.checkpoint_id,
        operation_id: init.operation_id,
        operation_identity_digest: init.operation_identity_digest,
        operation_class: init.operation_class,
        authorization_digest: init.authorization_digest,
        custody_digest: init.custody_digest,
        freeze_digest: init.freeze_digest,
        repository: init.repository,
        tag: init.tag,
        journal_head_sequence: init.journal_head_sequence,
        journal_head_digest: init.journal_head_digest,
        checkpoint_sequence: init.checkpoint_sequence,
        prior_checkpoint_digest,
        kind: init.kind,
        github_release_id: init.github_release_id,
        gated_mutation: init.gated_mutation,
        provider: init.provider,
        producer: init.producer,
        retention_days: init.retention_days,
        created_at_unix_seconds: init.created_at_unix_seconds,
        expires_at_unix_seconds: expected_expiry,
        note: init.note,
        readback: GitHubReleaseCheckpointReadbackV1::Missing,
        readback_at_unix_seconds: None,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_create_releases".to_string(),
            "does_not_upload_assets".to_string(),
            "does_not_authorize_operation".to_string(),
        ],
    })
}

/// Begin a checkpoint bound to one canonical #3940 release operation. The
/// init digests must equal the canonical identity's authority fields and the
/// class must agree; the identity itself is revalidated so a digest-shaped
/// foreign operation can never own a checkpoint.
pub fn begin_github_release_checkpoint_for_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    mut init: GitHubReleaseCheckpointInitV1,
) -> Result<CargoAllowGitHubReleaseCheckpointV1, &'static str> {
    validate_release_operation_identity_v1(identity)
        .map_err(|_| "checkpoint operation identity is not canonical")?;
    let class_agrees = matches!(
        (&identity.operation_class, &init.operation_class,),
        (
            CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            GitHubReleaseCheckpointClassV1::CleanFinalPublication,
        ) | (
            CargoAllowReleaseOperationClassV1::IncidentRecovery,
            GitHubReleaseCheckpointClassV1::IncidentRecovery,
        )
    );
    if !class_agrees {
        return Err("checkpoint class must agree with the canonical operation class");
    }
    if init.authorization_digest != identity.authorization_digest
        || init.custody_digest != identity.custody_digest
    {
        return Err("checkpoint authority fields must agree with the canonical operation");
    }
    let identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    init.operation_identity_digest = identity_digest.clone();
    let checkpoint = begin_github_release_checkpoint_v1(init, None)?;
    if checkpoint.operation_identity_digest != identity_digest {
        return Err("checkpoint must name the canonical operation");
    }
    Ok(checkpoint)
}

fn record_checkpoint_readback_internal_v1(
    checkpoint: &mut CargoAllowGitHubReleaseCheckpointV1,
    outcome: GitHubCheckpointProviderOutcomeV1,
    at_unix_seconds: u64,
) -> Result<
    (
        GitHubReleaseCheckpointReadbackV1,
        Option<GitHubReleaseCheckpointReadbackWitnessV1>,
    ),
    &'static str,
> {
    if at_unix_seconds < checkpoint.created_at_unix_seconds {
        return Err("readbacks must not predate checkpoint construction");
    }
    if checkpoint
        .readback_at_unix_seconds
        .is_some_and(|previous| at_unix_seconds < previous)
    {
        return Err("readback observations never move backward");
    }
    match outcome {
        GitHubCheckpointProviderOutcomeV1::Unavailable => {
            checkpoint.readback = GitHubReleaseCheckpointReadbackV1::ProviderUnavailable;
            checkpoint.readback_at_unix_seconds = Some(at_unix_seconds);
            Ok((GitHubReleaseCheckpointReadbackV1::ProviderUnavailable, None))
        }
        GitHubCheckpointProviderOutcomeV1::InstrumentFailure => {
            checkpoint.readback = GitHubReleaseCheckpointReadbackV1::InstrumentFailure;
            checkpoint.readback_at_unix_seconds = Some(at_unix_seconds);
            Ok((GitHubReleaseCheckpointReadbackV1::InstrumentFailure, None))
        }
        GitHubCheckpointProviderOutcomeV1::Delivered(bytes) => {
            Ok((classify_delivered_v1(checkpoint, &bytes), None))
        }
    }
}

/// Record an outage or instrument failure. Delivered bytes classify through
/// [`record_github_release_checkpoint_readback_with_witness_v1`].
pub fn record_github_release_checkpoint_readback_v1(
    checkpoint: &mut CargoAllowGitHubReleaseCheckpointV1,
    outcome: GitHubCheckpointProviderOutcomeV1,
    at_unix_seconds: u64,
) -> Result<GitHubReleaseCheckpointReadbackV1, &'static str> {
    let (readback, _) =
        record_checkpoint_readback_internal_v1(checkpoint, outcome, at_unix_seconds)?;
    Ok(readback)
}

/// Record delivered provider bytes with an opaque witness on `Complete`.
pub fn record_github_release_checkpoint_readback_with_witness_v1(
    checkpoint: &mut CargoAllowGitHubReleaseCheckpointV1,
    outcome: GitHubCheckpointProviderOutcomeV1,
    at_unix_seconds: u64,
) -> Result<
    (
        GitHubReleaseCheckpointReadbackV1,
        Option<GitHubReleaseCheckpointReadbackWitnessV1>,
    ),
    &'static str,
> {
    if at_unix_seconds < checkpoint.created_at_unix_seconds {
        return Err("readbacks must not predate checkpoint construction");
    }
    if checkpoint
        .readback_at_unix_seconds
        .is_some_and(|previous| at_unix_seconds < previous)
    {
        return Err("readback observations never move backward");
    }
    let GitHubCheckpointProviderOutcomeV1::Delivered(bytes) = outcome else {
        return record_checkpoint_readback_internal_v1(checkpoint, outcome, at_unix_seconds);
    };
    let verdict = classify_delivered_v1(checkpoint, &bytes);
    checkpoint.readback = verdict;
    checkpoint.readback_at_unix_seconds = Some(at_unix_seconds);
    if verdict != GitHubReleaseCheckpointReadbackV1::Complete {
        return Ok((verdict, None));
    }
    let witness = GitHubReleaseCheckpointReadbackWitnessV1 {
        checkpoint_link_digest: digest_github_release_checkpoint_link_v1(checkpoint)
            .map_err(|_| "readback witness digest failed")?,
        provider_object_id: checkpoint.provider.object_id.clone(),
        downloaded_bytes_digest: digest_github_release_checkpoint_bytes_v1(&bytes),
        observed_at_unix_seconds: at_unix_seconds,
    };
    Ok((verdict, Some(witness)))
}

fn same_checkpoint_subject_v1(
    expected: &CargoAllowGitHubReleaseCheckpointV1,
    observed: &CargoAllowGitHubReleaseCheckpointV1,
) -> bool {
    expected.schema_id == observed.schema_id
        && expected.schema_version == observed.schema_version
        && expected.operation_id == observed.operation_id
        && expected.operation_identity_digest == observed.operation_identity_digest
        && expected.operation_class == observed.operation_class
        && expected.authorization_digest == observed.authorization_digest
        && expected.custody_digest == observed.custody_digest
        && expected.freeze_digest == observed.freeze_digest
        && expected.repository == observed.repository
        && expected.tag == observed.tag
        && expected.producer == observed.producer
}

fn classify_delivered_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
    bytes: &[u8],
) -> GitHubReleaseCheckpointReadbackV1 {
    use GitHubReleaseCheckpointReadbackV1 as Readback;
    if bytes.is_empty() {
        return Readback::Mismatch;
    }
    let parsed: Result<CargoAllowGitHubReleaseCheckpointV1, _> = serde_json::from_slice(bytes);
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(_) => return Readback::Mismatch,
    };
    if !same_checkpoint_subject_v1(checkpoint, &parsed) {
        return Readback::Mismatch;
    }
    if parsed.checkpoint_sequence < checkpoint.checkpoint_sequence {
        return Readback::Stale;
    }
    if parsed.checkpoint_sequence != checkpoint.checkpoint_sequence {
        return Readback::Mismatch;
    }
    let canonical = match canonical_stored_checkpoint_bytes_v1(checkpoint) {
        Ok(bytes) => bytes,
        Err(_) => return Readback::Mismatch,
    };
    if bytes != canonical.as_slice() {
        return Readback::Mismatch;
    }
    if bytes.len() as u64 != checkpoint.provider.object_size_bytes {
        return Readback::Mismatch;
    }
    let body_digest = digest_github_release_checkpoint_body_v1(&parsed).unwrap_or_default();
    if body_digest != checkpoint.provider.object_digest {
        return Readback::Mismatch;
    }
    Readback::Complete
}

/// Select one checkpoint by exact identity. The provider object name never
/// participates: a same-name object from another run, producer, or operation
/// is never selected, no matter how recent it claims to be.
pub fn select_github_release_checkpoint_by_exact_identity_v1<'a>(
    candidates: &'a [CargoAllowGitHubReleaseCheckpointV1],
    object_id: &str,
    expected_producer: &GitHubReleaseCheckpointProducerV1,
    operation_id: &str,
    operation_identity_digest: &str,
) -> Result<Option<&'a CargoAllowGitHubReleaseCheckpointV1>, &'static str> {
    let mut matches = candidates.iter().filter(|candidate| {
        candidate.provider.object_id == object_id
            && candidate.producer == *expected_producer
            && candidate.operation_id == operation_id
            && candidate.operation_identity_digest == operation_identity_digest
    });
    let selected = matches.next();
    if matches.next().is_some() {
        return Err("checkpoint discovery by exact identity is ambiguous");
    }
    Ok(selected)
}

/// Verify a checkpoint against the live journal, the expected producer, the
/// opaque exact-byte readback witness, and the wall clock. Serialized
/// `readback=complete` fields are evidence only and never progress authority.
pub fn verify_github_release_checkpoint_against_journal_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
    witness: &GitHubReleaseCheckpointReadbackWitnessV1,
    journal: &CargoAllowGitHubReleaseJournalV1,
    expected_producer: &GitHubReleaseCheckpointProducerV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    verify_readback_witness_v1(checkpoint, witness)?;
    verify_github_release_journal_v1(journal)?;
    if checkpoint.journal_head_sequence < 1 {
        return Err("checkpoints bind a non-empty journal prefix");
    }
    if !journal.entries.iter().any(|entry| {
        entry.sequence == checkpoint.journal_head_sequence
            && entry.entry_digest == checkpoint.journal_head_digest
    }) {
        return Err("checkpoint journal prefix does not match the live journal");
    }
    if checkpoint.operation_identity_digest != journal.operation_identity_digest
        || checkpoint.operation_id != journal.operation_id
        || checkpoint.authorization_digest != journal.authorization_digest
        || checkpoint.custody_digest != journal.custody_digest
        || checkpoint.freeze_digest != journal.freeze_digest
        || checkpoint.repository != journal.repository
        || checkpoint.tag != journal.tag
    {
        return Err("checkpoint operation identity must match the verified journal header");
    }
    if checkpoint.producer.workflow != journal.workflow
        || checkpoint.producer.run != journal.run
        || checkpoint.producer.attempt != journal.attempt
        || checkpoint.producer.job != journal.job
    {
        return Err("checkpoint producer execution identity must match the journal");
    }
    if checkpoint.producer != *expected_producer {
        return Err("checkpoint producer must match the expected release producer");
    }
    if checkpoint.readback != GitHubReleaseCheckpointReadbackV1::Complete {
        return Err("provider success without readback is never clean");
    }
    let readback_at = checkpoint
        .readback_at_unix_seconds
        .ok_or("Complete readback requires an observation time")?;
    if readback_at < checkpoint.created_at_unix_seconds
        || readback_at > now_unix_seconds
        || readback_at >= checkpoint.expires_at_unix_seconds
    {
        return Err("checkpoint readback time is outside its trusted window");
    }
    if now_unix_seconds < checkpoint.created_at_unix_seconds
        || now_unix_seconds >= checkpoint.expires_at_unix_seconds
    {
        return Err("expired or premature checkpoints never authorize progress");
    }
    Ok(())
}

fn verify_readback_witness_v1(
    checkpoint: &CargoAllowGitHubReleaseCheckpointV1,
    witness: &GitHubReleaseCheckpointReadbackWitnessV1,
) -> Result<(), &'static str> {
    if witness.checkpoint_link_digest
        != digest_github_release_checkpoint_link_v1(checkpoint)
            .map_err(|_| "readback witness link digest failed")?
    {
        return Err("checkpoint readback witness does not bind this checkpoint");
    }
    if witness.provider_object_id != checkpoint.provider.object_id {
        return Err("checkpoint readback witness names another provider object");
    }
    if witness.observed_at_unix_seconds != checkpoint.readback_at_unix_seconds.unwrap_or(0) {
        return Err("checkpoint readback witness time does not match the observation");
    }
    let stored = canonical_stored_checkpoint_bytes_v1(checkpoint)
        .map_err(|_| "readback witness stored bytes failed")?;
    if digest_github_release_checkpoint_bytes_v1(&stored) != witness.downloaded_bytes_digest {
        return Err("checkpoint readback witness does not bind the exact downloaded bytes");
    }
    Ok(())
}
