//! Durable one-operation lease serializing clean and recovery release runs (#3925).
//!
//! Workflow concurrency by ref or workflow name is scheduling, not authority:
//! clean and recovery dispatches may use different refs, a cancelled runner
//! may leave uncertain external state, and a second run may begin while the
//! first authorization is only locally marked selected. This module models the
//! durable lease that must be acquired and independently read back before tag
//! creation, token access, or upload planning.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! credential reads, no tag creation, no uploads, and no live-state mutation.
//! Lease records contain no credential material and no raw authorization
//! payload, only identity and digest bindings.

use serde::{Deserialize, Serialize};

pub const OPERATION_LEASE_SCHEMA_ID: &str = "cargo-allow.release-operation-lease.v1";
pub const OPERATION_LEASE_SCHEMA_VERSION: u32 = 1;

/// The single clean final operation a lease may serialize.
pub const OPERATION_LEASE_FINAL_OPERATION: &str = "publish_cargo_allow_final_0_2_0";
/// The incident-bound recovery operation, serialized against clean runs.
pub const OPERATION_LEASE_RECOVERY_OPERATION: &str = "publish_cargo_allow_recovery_0_2_0";
pub const OPERATION_LEASE_FINAL_VERSION: &str = "0.2.0";
pub const OPERATION_LEASE_FINAL_TAG: &str = "v0.2.0";

/// Operation classes serialized against each other for one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationLeaseClassV1 {
    Clean,
    Recovery,
}

/// Durable lease states. Only `Available` may be acquired; only the Held
/// states authorize continuation; terminal states never authorize a clean
/// retry on their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationLeaseStateV1 {
    Available,
    HeldPreIrreversible,
    HeldIrreversible,
    ReleasedComplete,
    ReleasedIncident,
    ExpiredPreIrreversible,
    RecoveryRequired,
    Conflict,
    ProviderUnavailable,
    InstrumentFailure,
}

/// Exact release-subject identity the lease key derives from. Branch, ref,
/// and workflow text never enter the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationLeaseKeyV1 {
    pub operation: String,
    pub version: String,
    pub tag: String,
    pub commit: String,
    pub tree: String,
    pub denominator_digest: String,
}

/// Holder identity: one workflow run attempt. Run text identifies the holder;
/// it never authorizes the operation by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationLeaseHolderV1 {
    pub lease_id: String,
    pub generation: u64,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
}

/// One append-only lease transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationLeaseTransitionV1 {
    pub from: OperationLeaseStateV1,
    pub to: OperationLeaseStateV1,
    pub at_unix_seconds: u64,
    pub reason: String,
}

/// Durable lease record for one exact release operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseOperationLeaseV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub lease_id: String,
    pub class: OperationLeaseClassV1,
    pub key: OperationLeaseKeyV1,
    pub key_digest: String,
    pub subject_digest: String,
    pub holder: OperationLeaseHolderV1,
    pub acquired_at_unix_seconds: u64,
    pub renewed_at_unix_seconds: u64,
    pub renewals: u64,
    pub max_renewals: u64,
    pub expires_at_unix_seconds: u64,
    pub journal_head_digest: String,
    pub checkpoint_head_digest: String,
    pub first_irreversible_started: bool,
    pub state: OperationLeaseStateV1,
    pub transitions: Vec<OperationLeaseTransitionV1>,
    /// Always true: lease records carry no credential material.
    pub redacted: bool,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// Caller-supplied acquisition inputs. The observed lease is the independent
/// readback of current lease storage, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationLeaseAcquireInitV1 {
    pub lease_id: String,
    pub class: OperationLeaseClassV1,
    pub key: OperationLeaseKeyV1,
    pub holder_workflow: String,
    pub holder_run: String,
    pub holder_attempt: String,
    pub holder_job: String,
    pub journal_head_digest: String,
    pub checkpoint_head_digest: String,
    pub max_renewals: u64,
    pub acquired_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub storage_provider_available: bool,
}

const CLAIM_BOUNDARY: &str = "This record models a durable one-operation lease serializing clean and recovery release runs for one exact cargo-allow subject. It prevents concurrent or falsely restarted irreversible work; it does not authorize the operation, prove provider success, or execute publication.";

const SECRET_MARKERS: [&str; 12] = [
    "BEGIN PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN EC PRIVATE KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "ghp_",
    "github_pat_",
    "AKIA",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "password=",
    "token=",
];

/// Canonical lowercase hexadecimal: uppercase forms identify the same
/// object but hash to different key digests, so they are rejected rather
/// than normalized. All real producers (git, content digests) emit lowercase.
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

fn git_sha_shape(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && lower_hex_shape(value)
}

fn secret_marker(value: &str) -> bool {
    SECRET_MARKERS
        .into_iter()
        .any(|marker| value.contains(marker))
}

fn content_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

/// Canonical lease-key digest. The key binds exact operation identity:
/// any moved commit, tree, denominator, version, or operation class derives
/// another key.
pub fn operation_lease_key_digest_v1(
    key: &OperationLeaseKeyV1,
) -> Result<String, serde_json::Error> {
    content_digest(&(
        key.operation.as_str(),
        key.version.as_str(),
        key.tag.as_str(),
        key.commit.as_str(),
        key.tree.as_str(),
        key.denominator_digest.as_str(),
    ))
}

/// Subject-contention digest: the operation-excluded identity two dispatches
/// for one subject share (tag event or workflow dispatch, any ref; clean or
/// recovery class). Held-state, conflict, and generation checks compare this
/// digest so clean and recovery runs for one subject always serialize, while
/// the operation-specific key digest stays record metadata.
pub fn operation_lease_subject_digest_v1(
    key: &OperationLeaseKeyV1,
) -> Result<String, serde_json::Error> {
    content_digest(&(
        key.version.as_str(),
        key.tag.as_str(),
        key.commit.as_str(),
        key.tree.as_str(),
        key.denominator_digest.as_str(),
    ))
}

fn validate_lease_key(
    key: &OperationLeaseKeyV1,
    class: OperationLeaseClassV1,
) -> Result<(), &'static str> {
    let expected_operation = match class {
        OperationLeaseClassV1::Clean => OPERATION_LEASE_FINAL_OPERATION,
        OperationLeaseClassV1::Recovery => OPERATION_LEASE_RECOVERY_OPERATION,
    };
    if key.operation != expected_operation
        || key.version != OPERATION_LEASE_FINAL_VERSION
        || key.tag != OPERATION_LEASE_FINAL_TAG
    {
        return Err("lease key must bind the exact selected operation identity");
    }
    if !git_sha_shape(&key.commit) || !git_sha_shape(&key.tree) {
        return Err("lease key requires canonical commit and tree SHAs");
    }
    if !digest_shape(&key.denominator_digest) {
        return Err("lease key requires a well-formed denominator digest");
    }
    Ok(())
}

fn validate_holder(
    workflow: &str,
    run: &str,
    attempt: &str,
    job: &str,
) -> Result<(), &'static str> {
    for value in [workflow, run, attempt, job] {
        if value.trim().is_empty() || secret_marker(value) {
            return Err("lease holder identity must be complete and credential-free");
        }
    }
    Ok(())
}

/// Acquire the durable lease for one exact operation. An independently
/// read-back existing lease for the same key held in a live state refuses
/// with `Conflict`; clean and recovery classes never overlap. A lease for
/// another key is a foreign subject: it neither blocks nor authorizes this
/// acquisition. An expired pre-irreversible lease may be re-acquired at the
/// next generation; anything post-irreversible must reconcile first.
pub fn acquire_operation_lease_v1(
    init: OperationLeaseAcquireInitV1,
    observed: Option<&CargoAllowReleaseOperationLeaseV1>,
    now_unix_seconds: u64,
) -> Result<CargoAllowReleaseOperationLeaseV1, &'static str> {
    use OperationLeaseStateV1 as State;
    if !init.storage_provider_available {
        return Err("lease storage provider must be available at acquire time");
    }
    validate_lease_key(&init.key, init.class)?;
    validate_holder(
        &init.holder_workflow,
        &init.holder_run,
        &init.holder_attempt,
        &init.holder_job,
    )?;
    if secret_marker(&init.lease_id) {
        return Err("lease records must be credential-free");
    }
    if init.expires_at_unix_seconds <= init.acquired_at_unix_seconds
        || init.acquired_at_unix_seconds > now_unix_seconds
    {
        return Err("lease window must be ordered and already open");
    }
    for value in [
        init.journal_head_digest.as_str(),
        init.checkpoint_head_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("acquire requires well-formed journal and checkpoint head digests");
        }
    }
    let key_digest =
        operation_lease_key_digest_v1(&init.key).map_err(|_| "lease key digest failed")?;
    let subject_digest =
        operation_lease_subject_digest_v1(&init.key).map_err(|_| "lease subject digest failed")?;
    if init.expires_at_unix_seconds <= now_unix_seconds {
        return Err("acquire refuses an already-expired window");
    }
    let mut generation = 1;
    if let Some(existing) = observed
        && existing.subject_digest == subject_digest
    {
        match existing.state {
            State::HeldPreIrreversible | State::HeldIrreversible => {
                return Err("operation lease is already held");
            }
            State::ExpiredPreIrreversible => {
                if existing.first_irreversible_started {
                    return Err("post-irreversible expiry requires reconciliation");
                }
                generation = existing.holder.generation.saturating_add(1);
            }
            State::Available
            | State::ReleasedComplete
            | State::ReleasedIncident
            | State::RecoveryRequired
            | State::Conflict
            | State::ProviderUnavailable
            | State::InstrumentFailure => {
                return Err("observed lease state requires reconciliation first");
            }
        }
    }
    Ok(CargoAllowReleaseOperationLeaseV1 {
        schema_id: OPERATION_LEASE_SCHEMA_ID.to_string(),
        schema_version: OPERATION_LEASE_SCHEMA_VERSION,
        lease_id: init.lease_id.clone(),
        class: init.class,
        key: init.key,
        key_digest,
        subject_digest,
        holder: OperationLeaseHolderV1 {
            lease_id: init.lease_id,
            generation,
            workflow: init.holder_workflow,
            run: init.holder_run,
            attempt: init.holder_attempt,
            job: init.holder_job,
        },
        acquired_at_unix_seconds: init.acquired_at_unix_seconds,
        renewed_at_unix_seconds: init.acquired_at_unix_seconds,
        renewals: 0,
        max_renewals: init.max_renewals,
        expires_at_unix_seconds: init.expires_at_unix_seconds,
        journal_head_digest: init.journal_head_digest,
        checkpoint_head_digest: init.checkpoint_head_digest,
        first_irreversible_started: false,
        state: State::HeldPreIrreversible,
        transitions: vec![OperationLeaseTransitionV1 {
            from: State::Available,
            to: State::HeldPreIrreversible,
            at_unix_seconds: init.acquired_at_unix_seconds,
            reason: "acquired".to_string(),
        }],
        redacted: true,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_authorize_operation".to_string(),
            "does_not_prove_provider_success".to_string(),
            "workflow_concurrency_is_scheduling_only".to_string(),
        ],
    })
}

fn advance_lease_state(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    next: OperationLeaseStateV1,
    at_unix_seconds: u64,
    reason: &str,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    let legal = matches!(
        (record.state, next),
        (State::HeldPreIrreversible, State::HeldIrreversible)
            | (State::HeldPreIrreversible, State::ReleasedIncident)
            | (State::HeldPreIrreversible, State::ExpiredPreIrreversible)
            | (State::HeldPreIrreversible, State::RecoveryRequired)
            | (State::HeldIrreversible, State::ReleasedComplete)
            | (State::HeldIrreversible, State::ReleasedIncident)
            | (State::HeldIrreversible, State::RecoveryRequired)
            | (State::Available, State::ProviderUnavailable)
            | (State::HeldPreIrreversible, State::ProviderUnavailable)
            | (State::HeldIrreversible, State::ProviderUnavailable)
            | (_, State::Conflict)
    );
    if !legal {
        return Err("invalid operation lease transition");
    }
    if record
        .transitions
        .last()
        .is_some_and(|previous| at_unix_seconds < previous.at_unix_seconds)
    {
        return Err("lease transitions must not move backwards in time");
    }
    record.transitions.push(OperationLeaseTransitionV1 {
        from: record.state,
        to: next,
        at_unix_seconds,
        reason: reason.to_string(),
    });
    record.state = next;
    Ok(())
}

/// Verify independently read-back storage bytes against the acquired record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseReadbackV1 {
    Match,
    Mismatch,
    Malformed,
}

pub fn verify_lease_readback_v1(
    record: &CargoAllowReleaseOperationLeaseV1,
    readback_json: &[u8],
) -> LeaseReadbackV1 {
    let stored = match serde_json::to_vec(record) {
        Ok(bytes) => bytes,
        Err(_) => return LeaseReadbackV1::Malformed,
    };
    let parsed: serde_json::Value = match serde_json::from_slice(readback_json) {
        Ok(value) => value,
        Err(_) => return LeaseReadbackV1::Malformed,
    };
    let stored_value: serde_json::Value = match serde_json::from_slice(&stored) {
        Ok(value) => value,
        Err(_) => return LeaseReadbackV1::Malformed,
    };
    if parsed == stored_value {
        LeaseReadbackV1::Match
    } else {
        LeaseReadbackV1::Mismatch
    }
}

/// Bounded renewal: extends the window without rewriting holder, key, or
/// journal history. Renewal is refused past the bound, past expiry, after the
/// irreversible start frees nothing, and from any non-held state.
pub fn renew_operation_lease_v1(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    now_unix_seconds: u64,
    new_expires_at_unix_seconds: u64,
    storage_provider_available: bool,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    if !storage_provider_available {
        return Err("lease storage provider must be available at renew time");
    }
    if !matches!(
        record.state,
        State::HeldPreIrreversible | State::HeldIrreversible
    ) {
        return Err("only a held lease can be renewed");
    }
    if now_unix_seconds > record.expires_at_unix_seconds {
        return Err("an expired lease must re-acquire, not renew");
    }
    if record.renewals >= record.max_renewals {
        return Err("lease renewal bound reached");
    }
    if record
        .transitions
        .last()
        .is_some_and(|previous| now_unix_seconds < previous.at_unix_seconds)
    {
        return Err("lease renewal must not predate the latest transition");
    }
    if new_expires_at_unix_seconds <= record.expires_at_unix_seconds {
        return Err("renewal must extend the lease window");
    }
    if now_unix_seconds < record.renewed_at_unix_seconds {
        return Err("lease renewal must not move backwards in time");
    }
    record.renewals += 1;
    record.renewed_at_unix_seconds = now_unix_seconds;
    record.expires_at_unix_seconds = new_expires_at_unix_seconds;
    record.transitions.push(OperationLeaseTransitionV1 {
        from: record.state,
        to: record.state,
        at_unix_seconds: now_unix_seconds,
        reason: "renewed".to_string(),
    });
    Ok(())
}

/// Note the first irreversible action: the lease can never again become
/// available to a clean run on timeout or runner loss.
pub fn note_lease_irreversible_start_v1(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    if record.state != State::HeldPreIrreversible {
        return Err("only a pre-irreversible held lease can start");
    }
    if now_unix_seconds > record.expires_at_unix_seconds {
        return Err("an expired lease must re-acquire before starting");
    }
    advance_lease_state(record, State::HeldIrreversible, now_unix_seconds, "started")?;
    record.first_irreversible_started = true;
    Ok(())
}

/// Release a held lease as complete or incident.
pub fn release_operation_lease_v1(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    complete: bool,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    let next = if complete {
        State::ReleasedComplete
    } else {
        State::ReleasedIncident
    };
    advance_lease_state(
        record,
        next,
        now_unix_seconds,
        if complete {
            "released-complete"
        } else {
            "released-incident"
        },
    )
}

/// Cancel through the scheduling layer. Before the irreversible start the
/// holder steps aside into expiry (a clean run may re-acquire at the next
/// generation); after the start, cancellation cannot free the operation.
pub fn cancel_operation_lease_v1(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    match record.state {
        State::HeldPreIrreversible => advance_lease_state(
            record,
            State::ExpiredPreIrreversible,
            now_unix_seconds,
            "cancelled-pre-irreversible",
        ),
        State::HeldIrreversible => Err("cancellation cannot free an irreversible lease"),
        _ => Err("only a held lease can be cancelled"),
    }
}

/// Runner-loss evidence. Termination and handle release (fencing) must both
/// be attested: a missing session or heartbeat alone never expires a live
/// holder, or a stale observation could free the operation for a concurrent
/// runner. The #2502 execution lane supplies provider-verified evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerLossEvidenceV1 {
    pub holder_terminated: bool,
    pub handle_released: bool,
    pub observed_at_unix_seconds: u64,
}

/// Runner-loss observation. Loss after the irreversible start leaves a
/// recovery-required lease: no clean retry may acquire it. Loss before the
/// start expires the hold so a clean run may re-acquire.
pub fn observe_runner_loss_v1(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    evidence: RunnerLossEvidenceV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    if !evidence.holder_terminated || !evidence.handle_released {
        return Err("runner loss requires termination and handle-release evidence");
    }
    if evidence.observed_at_unix_seconds > now_unix_seconds {
        return Err("runner-loss evidence cannot be future-dated");
    }
    if record
        .transitions
        .last()
        .is_some_and(|previous| evidence.observed_at_unix_seconds < previous.at_unix_seconds)
    {
        return Err("stale runner-loss observation cannot expire the lease");
    }
    match record.state {
        State::HeldIrreversible => advance_lease_state(
            record,
            State::RecoveryRequired,
            now_unix_seconds,
            "runner-loss-post-irreversible",
        ),
        State::HeldPreIrreversible => advance_lease_state(
            record,
            State::ExpiredPreIrreversible,
            now_unix_seconds,
            "runner-loss-pre-irreversible",
        ),
        _ => Err("runner loss applies only to a held lease"),
    }
}

/// Record provider unavailability without ever reporting Available.
pub fn observe_lease_provider_unavailable_v1(
    record: &mut CargoAllowReleaseOperationLeaseV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use OperationLeaseStateV1 as State;
    match record.state {
        State::Available => advance_lease_state(
            record,
            State::ProviderUnavailable,
            now_unix_seconds,
            "provider-unavailable",
        ),
        State::HeldPreIrreversible | State::HeldIrreversible => advance_lease_state(
            record,
            State::ProviderUnavailable,
            now_unix_seconds,
            "provider-unavailable",
        ),
        _ => Err("provider observation applies only to live lease states"),
    }
}

/// Canonical JSON renderer for lease records.
pub fn render_release_operation_lease_v1(
    record: &CargoAllowReleaseOperationLeaseV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(record)
}
