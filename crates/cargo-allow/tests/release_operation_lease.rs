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
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationLeaseV1, CargoAllowReleaseOperationPackageRowV1, LeaseReadbackV1,
    OPERATION_LEASE_FINAL_OPERATION, OPERATION_LEASE_FINAL_TAG, OPERATION_LEASE_FINAL_VERSION,
    OPERATION_LEASE_RECOVERY_OPERATION, OPERATION_LEASE_SCHEMA_ID, OPERATION_LEASE_SCHEMA_VERSION,
    OperationLeaseAcquireInitV1, OperationLeaseClassV1, OperationLeaseKeyV1, OperationLeaseStateV1,
    RELEASE_AUTHORIZATION_SELECTION, RELEASE_OPERATION_ASSET_SELECTION, RunnerLossEvidenceV1,
    acquire_operation_lease_for_operation_v1, acquire_operation_lease_v1,
    build_release_operation_identity_v1, cancel_operation_lease_v1,
    note_lease_irreversible_start_v1, observe_lease_provider_unavailable_v1,
    observe_runner_loss_v1, operation_lease_key_digest_v1, operation_lease_subject_digest_v1,
    release_operation_identity_digest_v1, release_operation_lease_v1,
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
        operation_identity_digest: digest(77),
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

fn canonical_operation_identity(
    nonce: &str,
) -> Result<allow_report::CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
    let packages = RELEASE_AUTHORIZATION_SELECTION
        .iter()
        .filter(|(_, _, _, shared)| !*shared)
        .enumerate()
        .map(|(index, (logical_id, package_name, version, _))| {
            CargoAllowReleaseOperationPackageRowV1 {
                logical_id: (*logical_id).to_string(),
                package_name: (*package_name).to_string(),
                package_version: (*version).to_string(),
                package_digest: digest(100 + index as u64),
            }
        })
        .collect();
    let assets = RELEASE_OPERATION_ASSET_SELECTION
        .iter()
        .enumerate()
        .map(
            |(index, (asset_id, asset_name))| CargoAllowReleaseOperationAssetRowV1 {
                asset_id: (*asset_id).to_string(),
                asset_name: (*asset_name).to_string(),
                asset_digest: digest(200 + index as u64),
            },
        )
        .collect();
    Ok(
        build_release_operation_identity_v1(CargoAllowReleaseOperationIdentityInitV1 {
            nonce: nonce.to_string(),
            operation_class: CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            authority_kind: CargoAllowReleaseOperationAuthorityKindV1::Clean,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            product: "cargo-allow".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            github_prerelease: false,
            freeze_digest: digest(1),
            final_evidence_graph_digest: digest(2),
            custody_digest: digest(3),
            replay_digest: digest(4),
            authorization_digest: digest(5),
            cargo_lock_digest: digest(6),
            topology_digest: digest(7),
            support_digest: digest(8),
            channel_digest: digest(9),
            packages,
            assets,
            workflow_digest: digest(10),
            action_inventory_digest: digest(11),
            live_controls_digest: digest(12),
            incident_predecessor_operation_digest: None,
            incident_predecessor_head_digest: None,
            one_run_scope: true,
            expires_at_unix_seconds: 1_800_000_000,
        })
        .map_err(io::Error::other)?,
    )
}

#[test]
fn lease_binds_canonical_operation_identity() -> Result<(), Box<dyn Error>> {
    let identity = canonical_operation_identity("lease-binding-0001")?;
    let expected = release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?;
    let record =
        acquire_operation_lease_for_operation_v1(&identity, acquire_init(key()), None, NOW)
            .map_err(io::Error::other)?;
    require(
        record.key.operation_identity_digest == expected,
        "lease key must name the canonical operation identity digest",
    )?;

    // Same name, wrong operation: the key digest differs, and the foreign
    // run serializes against the held subject rather than acquiring over it.
    let foreign_identity = canonical_operation_identity("lease-binding-0002")?;
    let foreign_key_digest = operation_lease_key_digest_v1(
        &acquire_operation_lease_for_operation_v1(
            &foreign_identity,
            acquire_init(key()),
            None,
            NOW,
        )
        .map_err(io::Error::other)?
        .key,
    )
    .map_err(io::Error::other)?;
    require(
        foreign_key_digest
            != operation_lease_key_digest_v1(&record.key).map_err(io::Error::other)?,
        "same-name wrong-operation keys must never share a key digest",
    )?;
    require(
        acquire_operation_lease_for_operation_v1(
            &foreign_identity,
            acquire_init(key()),
            Some(&record),
            NOW,
        )
        .is_err(),
        "a second operation must not acquire over the held subject",
    )?;

    // Malformed digests fail closed without a canonical identity.
    let mut malformed = key();
    malformed.operation_identity_digest = "not-a-digest".to_string();
    require(
        acquire_operation_lease_v1(acquire_init(malformed), None, NOW).is_err(),
        "a non-canonical operation digest must fail closed",
    )?;

    // Class disagreement fails closed: a clean operation never holds a
    // recovery lease.
    let identity = canonical_operation_identity("lease-binding-0001")?;
    let mut crossed_class = acquire_init(key());
    crossed_class.class = allow_report::OperationLeaseClassV1::Recovery;
    require(
        acquire_operation_lease_for_operation_v1(&identity, crossed_class, None, NOW).is_err(),
        "lease class must agree with the canonical operation class",
    )?;
    let mut crossed_operation = acquire_init(key());
    crossed_operation.key.operation = OPERATION_LEASE_RECOVERY_OPERATION.to_string();
    require(
        acquire_operation_lease_for_operation_v1(&identity, crossed_operation, None, NOW).is_err(),
        "lease operation name must agree with the canonical operation class",
    )?;
    Ok(())
}

struct HeadFixture {
    identity: allow_report::CargoAllowReleaseOperationIdentityV1,
    producer: allow_report::ProducerIdentityV1,
    old_events: Vec<allow_report::CargoAllowReleaseOperationEventV1>,
    old_head: allow_report::CargoAllowReleaseOperationHeadV1,
    old_transfer: allow_report::CargoAllowReleaseArtifactTransferV1,
    old_files: Vec<allow_report::ActualDownloadedFileV1>,
    next_events: Vec<allow_report::CargoAllowReleaseOperationEventV1>,
    next_head: allow_report::CargoAllowReleaseOperationHeadV1,
    next_transfer: allow_report::CargoAllowReleaseArtifactTransferV1,
    next_files: Vec<allow_report::ActualDownloadedFileV1>,
    lease: CargoAllowReleaseOperationLeaseV1,
}

fn checkpoint_fixture(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
    events: &[allow_report::CargoAllowReleaseOperationEventV1],
    head: &allow_report::CargoAllowReleaseOperationHeadV1,
    producer: &allow_report::ProducerIdentityV1,
    id: &str,
) -> Result<(allow_report::CargoAllowReleaseArtifactTransferV1, Vec<allow_report::ActualDownloadedFileV1>), Box<dyn Error>> {
    use allow_report::*;
    let bytes = [
        ("identity.json", serde_json::to_vec(identity)?),
        ("events.json", serde_json::to_vec(events)?),
        ("head.json", serde_json::to_vec(head)?),
    ];
    let files: Vec<ArtifactTransferFileV1> = bytes.into_iter().map(|(path, data)| ArtifactTransferFileV1 {
        path: path.to_string(),
        size_bytes: data.len() as u64,
        sha256: allow_core::sha256_v1_bytes(&data).replacen("sha256:v1:", "sha256:", 1),
    }).collect();
    let downloaded = files.iter().map(|file| ActualDownloadedFileV1 {
        path: file.path.clone(), size_bytes: file.size_bytes, sha256: file.sha256.clone(),
    }).collect();
    let transfer = CargoAllowReleaseArtifactTransferV1::new(ArtifactTransferInitV1 {
        transfer_id: id.to_string(), role: OPERATION_LEASE_CHECKPOINT_ROLE.to_string(),
        stable_artifact_id: id.to_string(), producer: producer.clone(),
        provider_id: "github-actions".to_string(), provider_artifact_name: id.to_string(),
        files, semantic_payload_digest: Some(release_operation_head_digest_v1(head)?),
        trust_class: TrustClassV1::ManualDispatch,
        untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
        created_at_utc: "2026-08-06T10:00:00Z".to_string(),
    });
    Ok((transfer, downloaded))
}

fn head_fixture() -> Result<HeadFixture, Box<dyn Error>> {
    use allow_report::*;
    let identity = canonical_operation_identity("checked-heads-0001")?;
    let producer = ProducerIdentityV1 {
        repository: identity.repository.clone(), workflow_path: "release.yml".to_string(),
        git_ref: "refs/heads/frozen-final".to_string(), run_id: 101, run_attempt: 1,
        job_id: "publish".to_string(), commit_sha: key().commit, tree_sha: key().tree,
        release_version: "0.2.0".to_string(), tool_name: "cargo-allow".to_string(),
        schema_id: RELEASE_OPERATION_HEAD_SCHEMA_ID.to_string(), producer_generation: 1,
    };
    let event_producer = CargoAllowReleaseOperationProducerV1 {
        tool: producer.tool_name.clone(), schema: RELEASE_OPERATION_EVENT_SCHEMA_ID.to_string(),
        generation: 1, repository: producer.repository.clone(), workflow: producer.workflow_path.clone(),
        workflow_ref: producer.git_ref.clone(), run: producer.run_id.to_string(), attempt: 1,
        job: producer.job_id.clone(), commit: producer.commit_sha.clone(),
    };
    let init = |class, payload, at| CargoAllowReleaseOperationEventInitV1 {
        event_class: class, subject: CargoAllowReleaseOperationEventSubjectV1::Operation,
        payload_schema_id: RELEASE_OPERATION_IDENTITY_SCHEMA_ID.to_string(), payload_digest: payload,
        producer: event_producer.clone(), actor: "synthetic-maintainer".to_string(),
        authority_class: CargoAllowReleaseOperationAuthorityKindV1::Clean,
        request_boundary: "checked-heads".to_string(),
        response_posture: CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        semantic_result: CargoAllowReleaseOperationSemanticResultV1::Exact,
        artifact_digest: None, timestamp_source: CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
        observed_at_unix_seconds: at,
    };
    let mut old_events = Vec::new();
    let event = append_release_operation_event_v1(&identity, &old_events,
        init(CargoAllowReleaseOperationEventClassV1::OperationSelected,
             release_operation_identity_digest_v1(&identity)?, ACQUIRED_AT)).map_err(io::Error::other)?;
    old_events.push(event);
    let event = append_release_operation_event_v1(&identity, &old_events,
        init(CargoAllowReleaseOperationEventClassV1::AuthorizationSelected,
             identity.authorization_digest.clone(), ACQUIRED_AT + 1)).map_err(io::Error::other)?;
    old_events.push(event);
    let old_head = compile_release_operation_head_v1(&identity, &old_events, NOW).map_err(io::Error::other)?;
    let (old_transfer, old_files) = checkpoint_fixture(&identity, &old_events, &old_head, &producer, "checkpoint-101")?;
    let mut acquire = acquire_init(key());
    acquire.journal_head_digest = release_operation_head_digest_v1(&old_head)?;
    acquire.checkpoint_head_digest = allow_core::sha256_v1_bytes(&serde_json::to_vec(&old_transfer)?)
        .replacen("sha256:v1:", "sha256:", 1);
    let lease = acquire_operation_lease_for_operation_v1(&identity, acquire, None, NOW).map_err(io::Error::other)?;
    let mut next_events = old_events.clone();
    let event = append_release_operation_event_v1(&identity, &next_events,
        init(CargoAllowReleaseOperationEventClassV1::LeaseAcquired,
             allow_core::sha256_v1_bytes(&serde_json::to_vec(&lease)?).replacen("sha256:v1:", "sha256:", 1), NOW + 1))
        .map_err(io::Error::other)?;
    next_events.push(event);
    let next_head = compile_release_operation_head_v1(&identity, &next_events, NOW + 1).map_err(io::Error::other)?;
    let (next_transfer, next_files) = checkpoint_fixture(&identity, &next_events, &next_head, &producer, "checkpoint-102")?;
    Ok(HeadFixture { identity, producer, old_events, old_head, old_transfer, old_files,
        next_events, next_head, next_transfer, next_files, lease })
}

#[test]
fn checked_lease_head_advance_preserves_authority_and_rejects_false_readback() -> Result<(), Box<dyn Error>> {
    use allow_report::*;
    for control in ["exact", "wrong-old-head", "wrong-checkpoint", "no-op", "fork", "foreign-producer", "wrong-class", "generation", "expired", "bad-readback", "missing-file", "future-head"] {
        let mut fixture = head_fixture()?;
        let mut holder = fixture.lease.holder.clone();
        let mut now = NOW + 2;
        match control {
            "wrong-old-head" => fixture.lease.journal_head_digest = digest(998),
            "wrong-checkpoint" => fixture.lease.checkpoint_head_digest = digest(998),
            "no-op" => { fixture.next_events = fixture.old_events.clone(); fixture.next_head = fixture.old_head.clone(); }
            "fork" => { if let Some(event) = fixture.next_events.first_mut() { event.actor = "foreign".to_string(); } }
            "foreign-producer" => fixture.next_transfer.producer.run_id += 1,
            "wrong-class" => {
                fixture.lease.class = OperationLeaseClassV1::Recovery;
                fixture.lease.key.operation = OPERATION_LEASE_RECOVERY_OPERATION.to_string();
                fixture.lease.key_digest = operation_lease_key_digest_v1(&fixture.lease.key)?;
                fixture.lease.subject_digest = operation_lease_subject_digest_v1(&fixture.lease.key)?;
            }
            "generation" => holder.generation += 1,
            "expired" => now = EXPIRES_AT + 1,
            "missing-file" => { fixture.next_files.pop(); }
            "future-head" => fixture.next_head.evaluated_at_unix_seconds = now + 1,
            _ => {}
        }
        let before = fixture.lease.clone();
        let mut readback = serde_json::to_vec(&fixture.lease)?;
        if control == "bad-readback" { readback = b"{}".to_vec(); }
        let result = advance_operation_lease_heads_v1(&fixture.identity, &mut fixture.lease, OperationLeaseHeadAdvanceV1 {
            holder: &holder, producer: &fixture.producer, observed_lease_json: &readback,
            previous: OperationLeaseCheckpointReadbackV1 { history: &fixture.old_events, head: &fixture.old_head,
                transfer: &fixture.old_transfer, downloaded_files: &fixture.old_files },
            next: OperationLeaseCheckpointReadbackV1 { history: &fixture.next_events, head: &fixture.next_head,
                transfer: &fixture.next_transfer, downloaded_files: &fixture.next_files },
            now_unix_seconds: now,
        });
        if control == "exact" {
            result.map_err(io::Error::other)?;
            require(fixture.lease.journal_head_digest == release_operation_head_digest_v1(&fixture.next_head)?, "head advance did not retain the exact new head")?;
            require(fixture.lease.checkpoint_head_digest != before.checkpoint_head_digest, "head advance did not retain the new checkpoint")?;
            let mut unchanged = fixture.lease.clone();
            unchanged.journal_head_digest = before.journal_head_digest.clone();
            unchanged.checkpoint_head_digest = before.checkpoint_head_digest.clone();
            unchanged.transitions.pop();
            require(unchanged == before, "head advance changed holder, expiry, renewal or irreversible authority")?;
            // A new evaluation clock changes a head digest; retained checkpoints
            // must be verified at their own original evaluation time.
            let later = compile_release_operation_head_v1(&fixture.identity, &fixture.old_events, now).map_err(io::Error::other)?;
            require(release_operation_head_digest_v1(&later)? != release_operation_head_digest_v1(&fixture.old_head)?, "head clock control failed to distinguish stored bytes")?;
        } else {
            require(result.is_err() && fixture.lease == before, format!("{control} changed the lease or passed"))?;
        }
    }
    Ok(())
}
