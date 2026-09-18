//! Durable operation lease serializing clean and recovery runs (#3925).
//!
//! Synthetic subjects only: no tags are created, no registry is contacted,
//! and nothing leaves the process. These tests prove one-holder acquisition,
//! clean/recovery exclusion, bounded renewal, runner-loss semantics, and the
//! readback discipline the #2502/#2509 execution lanes will rely on.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use OperationLeaseStateV1 as State;
use allow_report::{
    CargoAllowReleaseOperationLeaseV1, LeaseReadbackV1, OPERATION_LEASE_FINAL_OPERATION,
    OPERATION_LEASE_FINAL_TAG, OPERATION_LEASE_FINAL_VERSION, OPERATION_LEASE_RECOVERY_OPERATION,
    OPERATION_LEASE_SCHEMA_ID, OPERATION_LEASE_SCHEMA_VERSION, OperationLeaseAcquireInitV1,
    OperationLeaseClassV1, OperationLeaseKeyV1, OperationLeaseStateV1, RunnerLossEvidenceV1,
    acquire_operation_lease_v1, cancel_operation_lease_v1, note_lease_irreversible_start_v1,
    observe_lease_provider_unavailable_v1, observe_runner_loss_v1, operation_lease_key_digest_v1,
    operation_lease_subject_digest_v1, release_operation_lease_v1,
    render_release_operation_lease_v1, renew_operation_lease_v1, verify_lease_readback_v1,
};

const ACQUIRED_AT: u64 = 1_786_100_000;
const EXPIRES_AT: u64 = 1_786_103_600;
const NOW: u64 = 1_786_100_100;

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

fn key() -> OperationLeaseKeyV1 {
    OperationLeaseKeyV1 {
        operation: OPERATION_LEASE_FINAL_OPERATION.to_string(),
        version: OPERATION_LEASE_FINAL_VERSION.to_string(),
        tag: OPERATION_LEASE_FINAL_TAG.to_string(),
        commit: "c".repeat(40),
        tree: "d".repeat(40),
        denominator_digest: digest(7),
    }
}

fn acquire_init(key: OperationLeaseKeyV1) -> OperationLeaseAcquireInitV1 {
    OperationLeaseAcquireInitV1 {
        lease_id: "lease-0-2-0-001".to_string(),
        class: OperationLeaseClassV1::Clean,
        key,
        holder_workflow: "release.yml".to_string(),
        holder_run: "101".to_string(),
        holder_attempt: "1".to_string(),
        holder_job: "publish".to_string(),
        journal_head_digest: digest(80),
        checkpoint_head_digest: digest(81),
        max_renewals: 3,
        acquired_at_unix_seconds: ACQUIRED_AT,
        expires_at_unix_seconds: EXPIRES_AT,
        storage_provider_available: true,
    }
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}

fn reacquire_init(key: OperationLeaseKeyV1, now: u64) -> OperationLeaseAcquireInitV1 {
    let mut init = acquire_init(key);
    init.acquired_at_unix_seconds = now;
    init.expires_at_unix_seconds = now + 3600;
    init
}

fn loss_evidence(at: u64) -> RunnerLossEvidenceV1 {
    RunnerLossEvidenceV1 {
        holder_terminated: true,
        handle_released: true,
        observed_at_unix_seconds: at,
    }
}

fn acquired() -> Result<CargoAllowReleaseOperationLeaseV1, Box<dyn Error>> {
    Ok(acquire_operation_lease_v1(acquire_init(key()), None, NOW).map_err(io::Error::other)?)
}

#[test]
fn release_operation_lease() -> Result<(), Box<dyn Error>> {
    let record = acquired()?;
    require(
        record.schema_id == OPERATION_LEASE_SCHEMA_ID
            && record.schema_version == OPERATION_LEASE_SCHEMA_VERSION,
        "lease record must carry the current generation",
    )?;
    require(
        record.state == State::HeldPreIrreversible
            && record.holder.generation == 1
            && record.redacted
            && !record.first_irreversible_started,
        "a fresh lease is held pre-irreversible at generation one",
    )?;
    require(
        record.key_digest == operation_lease_key_digest_v1(&record.key)?,
        "lease must bind its key digest",
    )?;
    require(
        record.claim_boundary.contains("does not authorize"),
        "lease lost its serialization-only claim boundary",
    )?;
    // Control: the key derives from subject identity, not ref text.
    let other_ref = key();
    require(
        operation_lease_key_digest_v1(&other_ref)? == record.key_digest,
        "two dispatches for one subject must derive one key",
    )?;
    let mut moved = key();
    moved.commit = "e".repeat(40);
    require(
        operation_lease_key_digest_v1(&moved)? != record.key_digest,
        "a moved commit must derive another key",
    )?;
    // Control: readback discipline.
    let rendered = render_release_operation_lease_v1(&record)?;
    require(
        verify_lease_readback_v1(&record, rendered.as_bytes()) == LeaseReadbackV1::Match,
        "exact stored bytes must read back as a match",
    )?;
    let forged = rendered.replace("lease-0-2-0-001", "lease-0-2-0-002");
    require(
        verify_lease_readback_v1(&record, forged.as_bytes()) == LeaseReadbackV1::Mismatch,
        "storage readback drift must report a mismatch",
    )?;
    require(
        verify_lease_readback_v1(&record, b"not json") == LeaseReadbackV1::Malformed,
        "non-JSON storage bytes must report malformed",
    )?;
    // Control: identity values are canonical lowercase hex; uppercase forms
    // identify the same object but hash to another key, so they are refused.
    let mut upper = key();
    upper.commit = "C".repeat(40);
    require(
        acquire_operation_lease_v1(acquire_init(upper), None, NOW).is_err(),
        "uppercase commit hex must fail closed",
    )?;
    let mut upper_digest = key();
    upper_digest.denominator_digest = format!("sha256:{:064X}", 0xABCDEF);
    require(
        acquire_operation_lease_v1(acquire_init(upper_digest), None, NOW).is_err(),
        "uppercase denominator hex must fail closed",
    )?;
    // Control: acquiring an already-expired window is refused.
    let mut stale_window = acquire_init(key());
    stale_window.acquired_at_unix_seconds = NOW - 7200;
    stale_window.expires_at_unix_seconds = NOW - 3600;
    match acquire_operation_lease_v1(stale_window, None, NOW) {
        Err(_) => {}
        Ok(record) => {
            return Err(format!(
                "stale window acquired: state={:?} acquired={} expires={} now={NOW}",
                record.state, record.acquired_at_unix_seconds, record.expires_at_unix_seconds
            )
            .into());
        }
    }
    // Control: renewal is bounded and preserves holder, key, and history.
    let mut record = acquired()?;
    let holder_before = record.holder.clone();
    renew_operation_lease_v1(&mut record, NOW + 10, EXPIRES_AT + 600, true)
        .map_err(io::Error::other)?;
    require(
        record.renewals == 1
            && record.expires_at_unix_seconds == EXPIRES_AT + 600
            && record.holder == holder_before,
        "renewal extends the window without rewriting identity",
    )?;
    renew_operation_lease_v1(&mut record, NOW + 20, EXPIRES_AT + 1200, true)
        .map_err(io::Error::other)?;
    renew_operation_lease_v1(&mut record, NOW + 30, EXPIRES_AT + 1800, true)
        .map_err(io::Error::other)?;
    require(
        renew_operation_lease_v1(&mut record, NOW + 40, EXPIRES_AT + 2400, true).is_err(),
        "renewal past the bound must fail",
    )?;
    // Control: lifecycle time never moves backwards.
    let mut record = acquired()?;
    note_lease_irreversible_start_v1(&mut record, NOW + 10).map_err(io::Error::other)?;
    require(
        release_operation_lease_v1(&mut record, true, NOW + 5).is_err()
            && record.state == State::HeldIrreversible,
        "settlement dated before the start must fail without mutating the lease",
    )?;
    renew_operation_lease_v1(&mut record, NOW + 20, EXPIRES_AT + 600, true)
        .map_err(io::Error::other)?;
    require(
        renew_operation_lease_v1(&mut record, NOW + 10, EXPIRES_AT + 1200, true).is_err(),
        "renewal dated before the last renewal must fail",
    )?;
    // Control: renewal after a later lifecycle transition is refused.
    let mut record = acquired()?;
    note_lease_irreversible_start_v1(&mut record, NOW + 30).map_err(io::Error::other)?;
    require(
        renew_operation_lease_v1(&mut record, NOW + 10, EXPIRES_AT + 600, true).is_err(),
        "renewal predating the irreversible start must fail",
    )?;
    // Control: lease records carry no credential material.
    let rendered = render_release_operation_lease_v1(&record)?;
    for marker in [
        "token=",
        "password=",
        "BEGIN PRIVATE KEY",
        "CARGO_REGISTRY_TOKEN",
    ] {
        require(
            !rendered.contains(marker),
            format!("lease artifact leaks secret marker {marker}"),
        )?;
    }
    Ok(())
}

#[test]
fn release_operation_lease_concurrency() -> Result<(), Box<dyn Error>> {
    // Control: two dispatches cannot acquire one operation lease.
    let first = acquired()?;
    require(
        acquire_operation_lease_v1(acquire_init(key()), Some(&first), NOW + 5).is_err(),
        "a second dispatch must conflict with the held lease",
    )?;
    // Control: clean and recovery runs never overlap on one subject: the
    // subject-contention digest excludes the operation class, so a held
    // clean lease conflicts recovery acquisition for the same subject.
    let mut recovery_init = acquire_init(key());
    recovery_init.class = OperationLeaseClassV1::Recovery;
    recovery_init.key.operation = OPERATION_LEASE_RECOVERY_OPERATION.to_string();
    recovery_init.lease_id = "lease-0-2-0-001-recovery".to_string();
    let recovery_key_digest = operation_lease_key_digest_v1(&recovery_init.key)?;
    require(
        recovery_key_digest != first.key_digest,
        "recovery keeps a distinct operation key digest",
    )?;
    require(
        operation_lease_subject_digest_v1(&recovery_init.key)? == first.subject_digest,
        "clean and recovery share one subject-contention digest",
    )?;
    require(
        acquire_operation_lease_v1(recovery_init, Some(&first), NOW + 5).is_err(),
        "recovery acquisition must conflict with the held clean lease",
    )?;
    // Control: a foreign subject neither blocks nor authorizes this key.
    let mut foreign = key();
    foreign.commit = "f".repeat(40);
    let foreign_record =
        acquire_operation_lease_v1(acquire_init(foreign), None, NOW).map_err(io::Error::other)?;
    let own = acquire_operation_lease_v1(acquire_init(key()), Some(&foreign_record), NOW + 5)
        .map_err(io::Error::other)?;
    require(
        own.state == State::HeldPreIrreversible,
        "a foreign lease must not block this subject",
    )?;
    // Control: expired pre-irreversible leases re-acquire at the next
    // generation; they are never treated as incidents.
    let mut record = acquired()?;
    cancel_operation_lease_v1(&mut record, NOW + 10).map_err(io::Error::other)?;
    require(
        record.state == State::ExpiredPreIrreversible,
        "pre-irreversible cancel must expire the hold",
    )?;
    let next = acquire_operation_lease_v1(
        reacquire_init(key(), EXPIRES_AT + 100),
        Some(&record),
        EXPIRES_AT + 100,
    )
    .map_err(io::Error::other)?;
    require(
        next.holder.generation == 2 && next.state == State::HeldPreIrreversible,
        "re-acquire after pre-irreversible expiry advances the generation",
    )?;
    // Control: cancellation cannot free an irreversible lease.
    let mut record = acquired()?;
    note_lease_irreversible_start_v1(&mut record, NOW + 10).map_err(io::Error::other)?;
    require(
        cancel_operation_lease_v1(&mut record, NOW + 20).is_err()
            && record.state == State::HeldIrreversible,
        "cancellation after the irreversible start must fail",
    )?;
    // Control: the holder cannot change without an append-only transition,
    // and the holder is immutable across renewals by construction.
    let mut record = acquired()?;
    let holder = record.holder.clone();
    renew_operation_lease_v1(&mut record, NOW + 10, EXPIRES_AT + 600, true)
        .map_err(io::Error::other)?;
    require(
        record.holder == holder,
        "renewal must preserve the holder identity",
    )?;
    // Control: provider failure is never Available.
    let mut init = acquire_init(key());
    init.storage_provider_available = false;
    require(
        acquire_operation_lease_v1(init, None, NOW).is_err(),
        "acquire against an unavailable provider must fail",
    )?;
    let mut record = acquired()?;
    require(
        renew_operation_lease_v1(&mut record, NOW + 10, EXPIRES_AT + 600, false).is_err(),
        "renew against an unavailable provider must fail",
    )?;
    observe_lease_provider_unavailable_v1(&mut record, NOW + 20).map_err(io::Error::other)?;
    require(
        record.state == State::ProviderUnavailable,
        "provider outage must be observed, never Available",
    )?;
    Ok(())
}

#[test]
fn release_operation_lease_runner_loss() -> Result<(), Box<dyn Error>> {
    // Control: runner loss after the irreversible start leaves a
    // recovery-required lease; no clean retry may acquire it.
    let mut record = acquired()?;
    note_lease_irreversible_start_v1(&mut record, NOW + 10).map_err(io::Error::other)?;
    require(
        record.first_irreversible_started && record.state == State::HeldIrreversible,
        "the irreversible start must be recorded on the lease",
    )?;
    observe_runner_loss_v1(&mut record, loss_evidence(NOW + 20), NOW + 20)
        .map_err(io::Error::other)?;
    require(
        record.state == State::RecoveryRequired,
        "post-irreversible runner loss must require recovery",
    )?;
    require(
        acquire_operation_lease_v1(acquire_init(key()), Some(&record), NOW + 30).is_err(),
        "no clean retry may acquire a recovery-required lease",
    )?;
    require(
        renew_operation_lease_v1(&mut record, NOW + 30, EXPIRES_AT + 600, true).is_err(),
        "a recovery-required lease must not renew",
    )?;
    // Control: runner loss before the start expires the hold for clean retry.
    let mut record = acquired()?;
    observe_runner_loss_v1(&mut record, loss_evidence(NOW + 10), NOW + 10)
        .map_err(io::Error::other)?;
    // Control: loss evidence without fencing never expires the lease.
    let mut record = acquired()?;
    let mut weak = loss_evidence(NOW + 10);
    weak.handle_released = false;
    require(
        observe_runner_loss_v1(&mut record, weak, NOW + 10).is_err()
            && record.state == State::HeldPreIrreversible,
        "unfenced loss observation must not expire the hold",
    )?;
    let stale = RunnerLossEvidenceV1 {
        holder_terminated: true,
        handle_released: true,
        observed_at_unix_seconds: ACQUIRED_AT - 1,
    };
    require(
        observe_runner_loss_v1(&mut record, stale, NOW + 10).is_err(),
        "stale loss observation must not expire the hold",
    )?;
    observe_runner_loss_v1(&mut record, loss_evidence(NOW + 10), NOW + 10)
        .map_err(io::Error::other)?;
    require(
        record.state == State::ExpiredPreIrreversible,
        "pre-irreversible runner loss must expire the hold",
    )?;
    let next = acquire_operation_lease_v1(
        reacquire_init(key(), EXPIRES_AT + 100),
        Some(&record),
        EXPIRES_AT + 100,
    )
    .map_err(io::Error::other)?;
    require(
        next.holder.generation == 2,
        "clean retry after pre-irreversible loss advances the generation",
    )?;
    // Control: starting requires a live held window.
    let mut record = acquired()?;
    require(
        note_lease_irreversible_start_v1(&mut record, EXPIRES_AT + 1).is_err(),
        "starting on an expired window must fail",
    )?;
    // Control: release settles held leases exactly once.
    let mut record = acquired()?;
    note_lease_irreversible_start_v1(&mut record, NOW + 10).map_err(io::Error::other)?;
    release_operation_lease_v1(&mut record, true, NOW + 20).map_err(io::Error::other)?;
    require(
        record.state == State::ReleasedComplete,
        "release must settle the lease",
    )?;
    require(
        release_operation_lease_v1(&mut record, true, NOW + 30).is_err(),
        "double release must fail",
    )?;
    require(
        acquire_operation_lease_v1(acquire_init(key()), Some(&record), NOW + 40).is_err(),
        "a settled lease requires reconciliation before any new hold",
    )
}

#[test]
fn rendered_lease_validates_against_json_schema() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.release-operation-lease.v1.schema.json"),
    )?)?;
    let rendered: serde_json::Value =
        serde_json::from_str(&render_release_operation_lease_v1(&acquired()?)?)?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| io::Error::other(format!("lease schema compiles: {error}")))?;
    validator.validate(&rendered).map_err(|error| {
        io::Error::other(format!("rendered lease record violates schema: {error}"))
    })?;
    for field in [
        "schema_id",
        "schema_version",
        "lease_id",
        "class",
        "key",
        "key_digest",
        "subject_digest",
        "holder",
        "state",
        "transitions",
        "redacted",
        "claim_boundary",
    ] {
        require(
            rendered.get(field).is_some(),
            format!("rendered lease dropped required field {field}"),
        )?;
    }
    require(
        rendered.get("redacted") == Some(&serde_json::Value::Bool(true)),
        "rendered lease must stay redacted",
    )
}
