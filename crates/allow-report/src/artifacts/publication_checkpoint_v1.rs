//! Remotely durable checkpoints for the publication journal (#3922).
//!
//! The #3921 journal survives process crashes on one runner; it does not
//! survive total hosted-runner loss. Before and after each irreversible
//! package row, the release operation needs one independently readable
//! remote checkpoint bound to the exact operation and journal prefix, so a
//! fresh runner can reconstruct the latest honest state instead of
//! re-inferring it.
//!
//! Substrate decision (retained in `docs/release/publication-checkpoint-v1.md`):
//! GitHub Actions Artifacts is the smallest adequate provider: per-upload
//! immutable under an exact numeric artifact ID, control-plane storage that
//! outlives any runner, configurable retention, and download-plus-digest
//! readback. Discovery is by exact object ID plus producer and operation
//! identity; artifact-name lookup across runs is forbidden because one name
//! identifies many runs. Cache, git refs, release assets, gists, and job
//! summaries were rejected for eviction, mutability, ambiguity, missing
//! producer identity, or unreadable bytes.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! uploads, no credential reads, no provider API calls, and no live-state
//! mutation. Provider outcomes are caller-supplied; the checkpoint classifies
//! them but never fetches them. A fresh checkpoint reads back as `Missing`
//! until an independent readback is recorded: provider success without
//! readback is never clean.

use serde::{Deserialize, Serialize};

use super::publication_journal_v1::{
    CargoAllowPublicationJournalV1, PUBLICATION_JOURNAL_OPERATION,
    PUBLICATION_JOURNAL_RECOVERY_OPERATION, PUBLICATION_JOURNAL_SCHEMA_ID,
    PublicationJournalClassV1, PublicationJournalEventV1, verify_publication_journal_v1,
};
use super::release_authorization_custody_v1::secret_marker;

pub const PUBLICATION_CHECKPOINT_SCHEMA_ID: &str = "cargo-allow.publication-checkpoint.v1";
pub const PUBLICATION_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

/// The journal schema every checkpoint binds.
pub const PUBLICATION_CHECKPOINT_JOURNAL_SCHEMA_ID: &str = PUBLICATION_JOURNAL_SCHEMA_ID;
/// Bounded operator note: leak prevention by construction, not screening.
pub const PUBLICATION_CHECKPOINT_MAX_NOTE_LEN: usize = 256;
/// GitHub artifact retention bounds, in days. Checkpoints must outlive the
/// release/recovery window and cannot outlive provider retention.
pub const PUBLICATION_CHECKPOINT_MIN_RETENTION_DAYS: u32 = 1;
pub const PUBLICATION_CHECKPOINT_MAX_RETENTION_DAYS: u32 = 90;
const RETENTION_SECONDS_PER_DAY: u64 = 86_400;

/// Checkpoint operation class, mirroring the #3921 journal vocabulary.
/// Clean checkpoints never become recovery checkpoints; recovery starts a new
/// checkpoint sequence bound to the original journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationCheckpointClassV1 {
    CleanFinalPublication,
    IncidentRecovery,
}

/// Checkpoint position in the row lifecycle. Pre-intent checkpoints gate the
/// upload; post-observation checkpoints gate dependant rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationCheckpointKindV1 {
    PreIntentDurable,
    PostObservation,
}

/// Producer-claimed row state at checkpoint time. A claim, not a proof: the
/// journal prefix check (not state equality) is the verification invariant,
/// because the journal honestly advances past the checkpoint after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationCheckpointRowStateV1 {
    IntentDurable,
    UploadStarted,
    ResponseObserved,
    ResponseUnknown,
    VisibleExact,
    VisibleConflict,
    ObservedAbsent,
    Waiting,
    Incident,
}

/// Durable provider substrate. Exactly one variant: the selected GitHub
/// Actions Artifact surface. New substrates require a new schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationCheckpointProviderV1 {
    GithubActionsArtifact,
}

/// Independent readback outcome. Only `Complete` authorizes progress.
/// `ProviderUnavailable` is an outage, never absence: absence is a journal
/// verdict with its own provider observation, never a checkpoint outage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationCheckpointReadbackV1 {
    Complete,
    Missing,
    Stale,
    Mismatch,
    ProviderUnavailable,
    InstrumentFailure,
}

/// The package row this checkpoint gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationCheckpointRowV1 {
    pub package_name: String,
    pub row_order: u32,
    pub state: PublicationCheckpointRowStateV1,
}

/// The remote provider object. `object_id` is the exact immutable provider
/// identity (the numeric artifact ID); `object_name` is a human label only
/// and never participates in discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationCheckpointProviderObjectV1 {
    pub provider: PublicationCheckpointProviderV1,
    pub object_id: String,
    pub object_name: String,
    pub object_digest: String,
    pub object_size_bytes: u64,
}

/// The producer identity the release lane expects. Equality with the
/// expected producer is the trust check: fork and untrusted jobs cannot
/// produce the protected workflow/run identity the release lane holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationCheckpointProducerV1 {
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
    pub git_ref: String,
    pub commit: String,
}

/// Opaque in-process witness that exact immutable provider bytes were
/// independently classified as Complete. It is deliberately absent from
/// serialized checkpoint records: deserializing a record that merely claims
/// `readback = complete` cannot manufacture gate authority.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicationCheckpointVerifiedReadbackV1 {
    checkpoint_digest: String,
    provider_object_id: String,
    downloaded_bytes_digest: String,
    readback_at_unix_seconds: u64,
}

/// One remotely durable publication checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowPublicationCheckpointV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub checkpoint_id: String,
    pub operation_id: String,
    pub operation_class: PublicationCheckpointClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub journal_schema_id: String,
    pub journal_head_sequence: u64,
    pub journal_head_digest: String,
    pub checkpoint_sequence: u64,
    /// Monotonic linkage: sequence 1 carries `None`; every later checkpoint
    /// carries the canonical digest of its predecessor.
    pub prior_checkpoint_digest: Option<String>,
    pub kind: PublicationCheckpointKindV1,
    pub row: PublicationCheckpointRowV1,
    /// The first row that crossed an irreversible request, once known.
    /// Incident posture is monotonic: once set or recorded, it never clears.
    pub first_irreversible_row: Option<String>,
    pub incident_recorded: bool,
    pub provider: PublicationCheckpointProviderObjectV1,
    pub producer: PublicationCheckpointProducerV1,
    pub retention_days: u32,
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub note: String,
    /// Fresh checkpoints start `Missing`: provider success without an
    /// independent readback is never clean.
    pub readback: PublicationCheckpointReadbackV1,
    pub readback_at_unix_seconds: Option<u64>,
    /// Runtime-only proof created by exact byte classification. Serialized
    /// claims never carry this witness and therefore never authorize gates.
    #[serde(skip)]
    verified_readback: Option<PublicationCheckpointVerifiedReadbackV1>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// Caller-supplied construction inputs. Linkage (`prior`) and readback are
/// assigned by the checkpoint, never the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationCheckpointInitV1 {
    pub checkpoint_id: String,
    pub operation_id: String,
    pub operation_class: PublicationCheckpointClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub journal_head_sequence: u64,
    pub journal_head_digest: String,
    pub checkpoint_sequence: u64,
    pub kind: PublicationCheckpointKindV1,
    pub row: PublicationCheckpointRowV1,
    pub first_irreversible_row: Option<String>,
    pub incident_recorded: bool,
    pub provider: PublicationCheckpointProviderObjectV1,
    pub producer: PublicationCheckpointProducerV1,
    pub retention_days: u32,
    pub created_at_unix_seconds: u64,
    pub note: String,
}

/// Caller-supplied provider outcome. Bytes are downloaded by the caller; the
/// checkpoint classifies them but never fetches them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointProviderOutcomeV1 {
    Delivered(Vec<u8>),
    Unavailable,
    InstrumentFailure,
}

const CLAIM_BOUNDARY: &str = "This record owns one remotely durable, independently readable checkpoint per publication journal prefix: exact operation and journal-prefix identity, monotonic sequence linkage, immutable provider object identity, producer trust, retention, and readback classification. It does not upload packages, observe the registry, authorize the operation, or execute recovery.";

/// Canonical lowercase hexadecimal, converged with the #3921 contract.
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

/// Canonical digest of checkpoint bytes, as retained in provider objects.
pub fn digest_publication_checkpoint_bytes_v1(bytes: &[u8]) -> String {
    checkpoint_digest_bytes(bytes)
}

/// Canonical JSON renderer for publication checkpoints.
pub fn render_publication_checkpoint_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(checkpoint)
}

fn immutable_checkpoint_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
) -> CargoAllowPublicationCheckpointV1 {
    let mut immutable = checkpoint.clone();
    immutable.readback = PublicationCheckpointReadbackV1::Missing;
    immutable.readback_at_unix_seconds = None;
    immutable.verified_readback = None;
    immutable
}

/// Canonical digest of the immutable stored checkpoint subject. Readback
/// observations never change sequence linkage, so a fresh runner hashing the
/// downloaded bytes derives the same predecessor identity.
pub fn digest_publication_checkpoint_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
) -> Result<String, serde_json::Error> {
    let immutable = immutable_checkpoint_v1(checkpoint);
    let rendered = serde_json::to_vec(&immutable)?;
    Ok(checkpoint_digest_bytes(&rendered))
}

/// The checkpoint body: every record field except the provider object and
/// the readback outcome. The provider object digest binds the body (not the
/// full record) so producers can compute the digest before storing: the
/// stored bytes carry their own body digest, and readback recomputes it.
#[derive(Serialize)]
struct PublicationCheckpointBodyV1<'a> {
    schema_id: &'a str,
    schema_version: u32,
    checkpoint_id: &'a str,
    operation_id: &'a str,
    operation_class: PublicationCheckpointClassV1,
    authorization_digest: &'a str,
    custody_digest: &'a str,
    freeze_digest: &'a str,
    journal_schema_id: &'a str,
    journal_head_sequence: u64,
    journal_head_digest: &'a str,
    checkpoint_sequence: u64,
    prior_checkpoint_digest: Option<&'a str>,
    kind: PublicationCheckpointKindV1,
    row: &'a PublicationCheckpointRowV1,
    first_irreversible_row: Option<&'a str>,
    incident_recorded: bool,
    producer: &'a PublicationCheckpointProducerV1,
    retention_days: u32,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    note: &'a str,
    claim_boundary: &'a str,
    limitations: &'a [String],
}

/// Canonical digest of the checkpoint body, as retained in provider objects.
pub fn digest_publication_checkpoint_body_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
) -> Result<String, serde_json::Error> {
    let body = PublicationCheckpointBodyV1 {
        schema_id: &checkpoint.schema_id,
        schema_version: checkpoint.schema_version,
        checkpoint_id: &checkpoint.checkpoint_id,
        operation_id: &checkpoint.operation_id,
        operation_class: checkpoint.operation_class,
        authorization_digest: &checkpoint.authorization_digest,
        custody_digest: &checkpoint.custody_digest,
        freeze_digest: &checkpoint.freeze_digest,
        journal_schema_id: &checkpoint.journal_schema_id,
        journal_head_sequence: checkpoint.journal_head_sequence,
        journal_head_digest: &checkpoint.journal_head_digest,
        checkpoint_sequence: checkpoint.checkpoint_sequence,
        prior_checkpoint_digest: checkpoint.prior_checkpoint_digest.as_deref(),
        kind: checkpoint.kind,
        row: &checkpoint.row,
        first_irreversible_row: checkpoint.first_irreversible_row.as_deref(),
        incident_recorded: checkpoint.incident_recorded,
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

fn validate_row(row: &PublicationCheckpointRowV1) -> Result<(), &'static str> {
    if row.package_name.trim().is_empty() {
        return Err("checkpoint rows require a package name");
    }
    Ok(())
}

fn validate_producer(producer: &PublicationCheckpointProducerV1) -> Result<(), &'static str> {
    for value in [
        producer.workflow.as_str(),
        producer.run.as_str(),
        producer.attempt.as_str(),
        producer.job.as_str(),
        producer.git_ref.as_str(),
        producer.commit.as_str(),
    ] {
        if value.trim().is_empty() {
            return Err("checkpoints require exact producer identity");
        }
    }
    if producer.commit.len() != 40 || !lower_hex_shape(&producer.commit) {
        return Err("checkpoint producer commits require canonical 40-byte lowercase hex");
    }
    if !producer.git_ref.starts_with("refs/") {
        return Err("checkpoint producer refs require an exact refs/ identity");
    }
    Ok(())
}

/// Begin a checkpoint bound to one operation and journal prefix. `prior`
/// carries the predecessor in the same operation when `checkpoint_sequence`
/// exceeds one; linkage, incident monotonicity, and journal-prefix
/// monotonicity are enforced here so a later clean record can never
/// overwrite incident history.
pub fn begin_publication_checkpoint_v1(
    init: PublicationCheckpointInitV1,
    prior: Option<&CargoAllowPublicationCheckpointV1>,
) -> Result<CargoAllowPublicationCheckpointV1, &'static str> {
    if init.checkpoint_id.trim().is_empty() {
        return Err("checkpoints require checkpoint identity");
    }
    let expected_operation = match init.operation_class {
        PublicationCheckpointClassV1::CleanFinalPublication => PUBLICATION_JOURNAL_OPERATION,
        PublicationCheckpointClassV1::IncidentRecovery => PUBLICATION_JOURNAL_RECOVERY_OPERATION,
    };
    if init.operation_id != expected_operation {
        return Err("checkpoint operation identity must match its class");
    }
    for value in [
        init.authorization_digest.as_str(),
        init.custody_digest.as_str(),
        init.freeze_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("checkpoints require canonical operation identity digests");
        }
    }
    if init.journal_head_sequence < 1 {
        return Err("checkpoints bind a non-empty journal prefix");
    }
    if !digest_shape(&init.journal_head_digest) {
        return Err("checkpoints bind a canonical journal head digest");
    }
    if init.checkpoint_sequence < 1 {
        return Err("checkpoint sequence starts at one");
    }
    validate_row(&init.row)?;
    if init.incident_recorded && init.first_irreversible_row.is_none() {
        return Err("incident posture requires its first irreversible row");
    }
    if let Some(first) = init.first_irreversible_row.as_deref()
        && first.trim().is_empty()
    {
        return Err("the first irreversible row must name a package");
    }
    if init.kind == PublicationCheckpointKindV1::PostObservation
        && init.first_irreversible_row.is_none()
    {
        return Err("post-observation checkpoints require the first irreversible row");
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
    if init.retention_days < PUBLICATION_CHECKPOINT_MIN_RETENTION_DAYS
        || init.retention_days > PUBLICATION_CHECKPOINT_MAX_RETENTION_DAYS
    {
        return Err("checkpoint retention must fit the provider window");
    }
    if init.created_at_unix_seconds < 1 {
        return Err("checkpoints require a positive construction time");
    }
    let retention_seconds = u64::from(init.retention_days)
        .checked_mul(RETENTION_SECONDS_PER_DAY)
        .ok_or("checkpoint retention overflow")?;
    let expected_expiry = init
        .created_at_unix_seconds
        .checked_add(retention_seconds)
        .ok_or("checkpoint expiry overflow")?;
    if init.note.len() > PUBLICATION_CHECKPOINT_MAX_NOTE_LEN {
        return Err("checkpoint notes are bounded and carry no bodies");
    }
    if secret_marker(&init.note).is_some() {
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
                    init.authorization_digest.as_str(),
                    previous.authorization_digest.as_str(),
                ),
                (
                    init.custody_digest.as_str(),
                    previous.custody_digest.as_str(),
                ),
                (init.freeze_digest.as_str(), previous.freeze_digest.as_str()),
            ] {
                if current != bound {
                    return Err("checkpoint linkage never crosses operations");
                }
            }
            if init.operation_class != previous.operation_class {
                return Err("checkpoint linkage never crosses operation classes");
            }
            if init.provider.object_id == previous.provider.object_id {
                return Err("checkpoint sequences require a new immutable provider object");
            }
            if init.created_at_unix_seconds < previous.created_at_unix_seconds {
                return Err("checkpoint construction time never moves backward");
            }
            if init.journal_head_sequence < previous.journal_head_sequence {
                return Err("checkpoint journal prefixes never move backward");
            }
            if previous.incident_recorded && !init.incident_recorded {
                return Err("incident history is never overwritten by later checkpoints");
            }
            if let Some(first) = previous.first_irreversible_row.as_deref()
                && init.first_irreversible_row.as_deref() != Some(first)
            {
                return Err("the first irreversible row never changes");
            }
            Some(
                digest_publication_checkpoint_v1(previous)
                    .map_err(|_| "checkpoint linkage digest failed")?,
            )
        }
    };
    Ok(CargoAllowPublicationCheckpointV1 {
        schema_id: PUBLICATION_CHECKPOINT_SCHEMA_ID.to_string(),
        schema_version: PUBLICATION_CHECKPOINT_SCHEMA_VERSION,
        checkpoint_id: init.checkpoint_id,
        operation_id: init.operation_id,
        operation_class: init.operation_class,
        authorization_digest: init.authorization_digest,
        custody_digest: init.custody_digest,
        freeze_digest: init.freeze_digest,
        journal_schema_id: PUBLICATION_CHECKPOINT_JOURNAL_SCHEMA_ID.to_string(),
        journal_head_sequence: init.journal_head_sequence,
        journal_head_digest: init.journal_head_digest,
        checkpoint_sequence: init.checkpoint_sequence,
        prior_checkpoint_digest,
        kind: init.kind,
        row: init.row,
        first_irreversible_row: init.first_irreversible_row,
        incident_recorded: init.incident_recorded,
        provider: init.provider,
        producer: init.producer,
        retention_days: init.retention_days,
        created_at_unix_seconds: init.created_at_unix_seconds,
        expires_at_unix_seconds: expected_expiry,
        note: init.note,
        readback: PublicationCheckpointReadbackV1::Missing,
        readback_at_unix_seconds: None,
        verified_readback: None,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_upload_packages".to_string(),
            "does_not_observe_registry".to_string(),
            "does_not_authorize_operation".to_string(),
        ],
    })
}

/// Record an independent readback of the stored provider bytes. Latest
/// observation wins; only `Complete` authorizes progress.
pub fn record_checkpoint_readback_v1(
    checkpoint: &mut CargoAllowPublicationCheckpointV1,
    outcome: CheckpointProviderOutcomeV1,
    at_unix_seconds: u64,
) -> Result<PublicationCheckpointReadbackV1, &'static str> {
    if at_unix_seconds < checkpoint.created_at_unix_seconds {
        return Err("readbacks must not predate checkpoint construction");
    }
    let (readback, downloaded_bytes_digest) = match outcome {
        CheckpointProviderOutcomeV1::Unavailable => {
            (PublicationCheckpointReadbackV1::ProviderUnavailable, None)
        }
        CheckpointProviderOutcomeV1::InstrumentFailure => {
            (PublicationCheckpointReadbackV1::InstrumentFailure, None)
        }
        CheckpointProviderOutcomeV1::Delivered(bytes) => {
            let bytes_digest = checkpoint_digest_bytes(&bytes);
            (
                classify_delivered_v1(checkpoint, &bytes),
                Some(bytes_digest),
            )
        }
    };
    checkpoint.readback = readback;
    checkpoint.readback_at_unix_seconds = Some(at_unix_seconds);
    checkpoint.verified_readback = match (readback, downloaded_bytes_digest) {
        (PublicationCheckpointReadbackV1::Complete, Some(bytes_digest)) => {
            Some(PublicationCheckpointVerifiedReadbackV1 {
                checkpoint_digest: digest_publication_checkpoint_v1(checkpoint)
                    .map_err(|_| "checkpoint readback seal digest failed")?,
                provider_object_id: checkpoint.provider.object_id.clone(),
                downloaded_bytes_digest: bytes_digest,
                readback_at_unix_seconds: at_unix_seconds,
            })
        }
        _ => None,
    };
    Ok(readback)
}

/// Classify exact downloaded provider bytes. The stored checkpoint is the
/// immutable Missing/no-readback subject; a serialized record that claims its
/// own successful readback is a mismatch, never authority.
fn classify_delivered_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    bytes: &[u8],
) -> PublicationCheckpointReadbackV1 {
    use PublicationCheckpointReadbackV1 as Readback;
    if bytes.is_empty() {
        return Readback::Mismatch;
    }
    let parsed: CargoAllowPublicationCheckpointV1 = match serde_json::from_slice(bytes) {
        Ok(parsed) => parsed,
        Err(_) => return Readback::Mismatch,
    };
    if parsed.operation_id != checkpoint.operation_id {
        return Readback::Mismatch;
    }
    if parsed.checkpoint_sequence < checkpoint.checkpoint_sequence {
        return Readback::Stale;
    }
    if parsed.checkpoint_sequence != checkpoint.checkpoint_sequence {
        return Readback::Mismatch;
    }
    let expected = immutable_checkpoint_v1(checkpoint);
    if parsed != expected {
        return Readback::Mismatch;
    }
    if bytes.len() as u64 != expected.provider.object_size_bytes {
        return Readback::Mismatch;
    }
    let canonical = match serde_json::to_vec_pretty(&expected) {
        Ok(canonical) => canonical,
        Err(_) => return Readback::InstrumentFailure,
    };
    if canonical.as_slice() != bytes {
        return Readback::Mismatch;
    }
    let body_digest = digest_publication_checkpoint_body_v1(&expected).unwrap_or_default();
    if body_digest != expected.provider.object_digest {
        return Readback::Mismatch;
    }
    Readback::Complete
}

/// Select one checkpoint by exact identity. The provider object name never
/// participates. Duplicate exact matches are ambiguous and therefore return
/// `None` rather than selecting whichever provider record happened to appear
/// first.
pub fn select_checkpoint_by_exact_identity_v1<'a>(
    candidates: &'a [CargoAllowPublicationCheckpointV1],
    object_id: &str,
    expected_producer: &PublicationCheckpointProducerV1,
    operation_id: &str,
) -> Option<&'a CargoAllowPublicationCheckpointV1> {
    let mut matches = candidates.iter().filter(|candidate| {
        candidate.provider.object_id == object_id
            && candidate.producer == *expected_producer
            && candidate.operation_id == operation_id
    });
    let selected = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(selected)
    }
}

/// Verify one checkpoint as authority over one exact, already-verified journal
/// transition. A Complete string in serialized input is insufficient: the
/// opaque readback seal must have been created by exact downloaded-byte
/// classification in this process.
pub fn verify_checkpoint_against_journal_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    journal: &CargoAllowPublicationJournalV1,
    expected_producer: &PublicationCheckpointProducerV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    verify_publication_journal_v1(journal)?;
    if checkpoint.schema_id != PUBLICATION_CHECKPOINT_SCHEMA_ID
        || checkpoint.schema_version != PUBLICATION_CHECKPOINT_SCHEMA_VERSION
        || checkpoint.journal_schema_id != PUBLICATION_CHECKPOINT_JOURNAL_SCHEMA_ID
        || journal.schema_id != PUBLICATION_CHECKPOINT_JOURNAL_SCHEMA_ID
    {
        return Err("checkpoint and journal schema identities must be exact");
    }
    if checkpoint.operation_id != journal.operation_id {
        return Err("checkpoints verify against their own operation only");
    }
    let expected_journal_class = match checkpoint.operation_class {
        PublicationCheckpointClassV1::CleanFinalPublication => {
            PublicationJournalClassV1::CleanFinalPublication
        }
        PublicationCheckpointClassV1::IncidentRecovery => {
            PublicationJournalClassV1::IncidentRecovery
        }
    };
    if journal.operation_class != expected_journal_class {
        return Err("checkpoint and journal operation classes must agree");
    }
    if checkpoint.authorization_digest != journal.authorization_digest
        || checkpoint.custody_digest != journal.custody_digest
        || checkpoint.freeze_digest != journal.freeze_digest
    {
        return Err("checkpoint operation identities must match the journal header");
    }
    if checkpoint.producer != *expected_producer {
        return Err("checkpoint producer must match the expected release producer");
    }
    let seal = checkpoint
        .verified_readback
        .as_ref()
        .ok_or("serialized readback claims never authorize progress")?;
    let readback_at = checkpoint
        .readback_at_unix_seconds
        .ok_or("complete readbacks require an observation time")?;
    if checkpoint.readback != PublicationCheckpointReadbackV1::Complete
        || seal.readback_at_unix_seconds != readback_at
        || seal.provider_object_id != checkpoint.provider.object_id
        || !digest_shape(&seal.downloaded_bytes_digest)
        || seal.checkpoint_digest
            != digest_publication_checkpoint_v1(checkpoint)
                .map_err(|_| "checkpoint readback seal digest failed")?
    {
        return Err("checkpoint readback authority does not match the immutable subject");
    }
    if readback_at < checkpoint.created_at_unix_seconds
        || readback_at >= checkpoint.expires_at_unix_seconds
        || readback_at > now_unix_seconds
    {
        return Err("checkpoint readback time must be current and inside retention");
    }
    if now_unix_seconds < checkpoint.created_at_unix_seconds
        || now_unix_seconds >= checkpoint.expires_at_unix_seconds
    {
        return Err("expired or premature checkpoints never authorize progress");
    }
    let body_digest = digest_publication_checkpoint_body_v1(checkpoint)
        .map_err(|_| "checkpoint body digest failed")?;
    if body_digest != checkpoint.provider.object_digest {
        return Err("checkpoint provider object digest must bind its immutable body");
    }
    let entry_index = checkpoint
        .journal_head_sequence
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or("checkpoint journal sequence is outside the local index")?;
    let bound = journal
        .entries
        .get(entry_index)
        .filter(|entry| entry.sequence == checkpoint.journal_head_sequence)
        .filter(|entry| entry.entry_digest == checkpoint.journal_head_digest)
        .ok_or("checkpoint journal prefix does not match the verified journal")?;
    let declared = journal
        .rows
        .iter()
        .find(|row| {
            row.package_name == checkpoint.row.package_name
                && row.row_order == checkpoint.row.row_order
        })
        .ok_or("checkpoint row must belong to the journal denominator")?;
    if bound.package_name.as_deref() != Some(declared.package_name.as_str())
        || bound.row_order != Some(declared.row_order)
        || bound.candidate_archive_digest.as_deref()
            != Some(declared.candidate_archive_digest.as_str())
    {
        return Err("checkpoint must bind the exact journal row transition");
    }
    match (checkpoint.kind, checkpoint.row.state, bound.kind) {
        (
            PublicationCheckpointKindV1::PreIntentDurable,
            PublicationCheckpointRowStateV1::IntentDurable,
            PublicationJournalEventV1::UploadIntentDurable,
        )
        | (
            PublicationCheckpointKindV1::PostObservation,
            PublicationCheckpointRowStateV1::VisibleExact,
            PublicationJournalEventV1::RegistryVisibleExact,
        ) => {}
        _ => {
            return Err(
                "checkpoint kind and row state must match the exact clean journal transition",
            );
        }
    }
    let prefix_first_irreversible = journal
        .entries
        .iter()
        .take(entry_index + 1)
        .find(|entry| entry.kind == PublicationJournalEventV1::UploadRequestStarted)
        .and_then(|entry| entry.package_name.as_deref());
    if checkpoint.first_irreversible_row.as_deref() != prefix_first_irreversible {
        return Err("checkpoint first irreversible row must match its journal prefix");
    }
    if checkpoint.incident_recorded != journal_has_incident(journal) {
        return Err("checkpoint incident posture must match the journal");
    }
    Ok(())
}

fn journal_has_incident(journal: &CargoAllowPublicationJournalV1) -> bool {
    journal
        .entries
        .iter()
        .any(|entry| entry.kind == PublicationJournalEventV1::OperationIncident)
}

/// A read-back pre-intent checkpoint authorizes exactly one upload to begin,
/// only while its durable intent is still the current journal head. Any
/// journal advancement consumes that authority and prevents replay.
pub fn checkpoint_permits_upload_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    journal: &CargoAllowPublicationJournalV1,
    expected_producer: &PublicationCheckpointProducerV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    verify_checkpoint_against_journal_v1(checkpoint, journal, expected_producer, now_unix_seconds)?;
    if checkpoint.kind != PublicationCheckpointKindV1::PreIntentDurable {
        return Err("only a pre-intent durable checkpoint may authorize an upload");
    }
    let bound_len = usize::try_from(checkpoint.journal_head_sequence)
        .map_err(|_| "checkpoint journal sequence is outside the local index")?;
    if journal.entries.len() != bound_len {
        return Err(
            "upload authority is consumed when the journal advances past the durable intent",
        );
    }
    Ok(())
}

/// A read-back post-observation checkpoint unlocks a dependant only while
/// the bound row remains the latest event for that package and the operation
/// has not completed or entered incident after the checkpoint prefix. Events
/// for other package rows may advance normally.
pub fn checkpoint_permits_dependant_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    journal: &CargoAllowPublicationJournalV1,
    expected_producer: &PublicationCheckpointProducerV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    verify_checkpoint_against_journal_v1(checkpoint, journal, expected_producer, now_unix_seconds)?;
    if checkpoint.kind != PublicationCheckpointKindV1::PostObservation {
        return Err("only a post-observation checkpoint may unlock a dependant");
    }
    let entry_index = checkpoint
        .journal_head_sequence
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or("checkpoint journal sequence is outside the local index")?;
    for entry in journal.entries.iter().skip(entry_index + 1) {
        if matches!(
            entry.kind,
            PublicationJournalEventV1::OperationIncident
                | PublicationJournalEventV1::OperationComplete
        ) {
            return Err("operation incident or completion consumes dependant authority");
        }
        if entry.package_name.as_deref() == Some(checkpoint.row.package_name.as_str()) {
            return Err("a later event for the checkpoint row requires a newer checkpoint");
        }
    }
    Ok(())
}
