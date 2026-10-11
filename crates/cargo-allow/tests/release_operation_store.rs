//! Existing Rust domain records through the production Python provider adapter.
//!
//! Only the HTTP boundary is intercepted. These are synthetic records and
//! provider responses, not a real authorization, storage observation, tag
//! transaction, or release gate. The fixture uses existing reducers and leaves
//! the lease/tag subject comparison and checked head-rebinding work with #3930.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use allow_report::{
    ActualDownloadedFileV1, ArtifactTransferDispositionV1, ArtifactTransferFileV1,
    ArtifactTransferInitV1, AuthorizationCustodyMintInitV1, CargoAllowReleaseArtifactTransferV1,
    CargoAllowReleaseAuthorizationConsumptionV1, CargoAllowReleaseAuthorizationCustodyV1,
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationEventClassV1,
    CargoAllowReleaseOperationEventInitV1, CargoAllowReleaseOperationEventSubjectV1,
    CargoAllowReleaseOperationEventV1, CargoAllowReleaseOperationHeadV1,
    CargoAllowReleaseOperationIdentityInitV1, CargoAllowReleaseOperationIdentityV1,
    CargoAllowReleaseOperationLeaseV1, CargoAllowReleaseOperationPackageRowV1,
    CargoAllowReleaseOperationProducerV1, CargoAllowReleaseOperationResponsePostureV1,
    CargoAllowReleaseOperationSemanticResultV1, CargoAllowReleaseOperationTimestampSourceV1,
    ConsumerContextV1, CustodyReadbackV1, FinalRegistryPreflightResultV1, LeaseReadbackV1,
    OPERATION_LEASE_FINAL_OPERATION, OPERATION_LEASE_FINAL_TAG, OPERATION_LEASE_FINAL_VERSION,
    OPERATION_LEASE_SCHEMA_ID, OperationLeaseAcquireInitV1, OperationLeaseClassV1,
    OperationLeaseKeyV1, ProducerIdentityV1, RELEASE_AUTHORIZATION_AUTH_CLASS,
    RELEASE_AUTHORIZATION_EXACT_STATEMENT, RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID,
    RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_VERSION, RELEASE_AUTHORIZATION_FINAL_OPERATION,
    RELEASE_AUTHORIZATION_FINAL_TAG, RELEASE_AUTHORIZATION_FINAL_VERSION,
    RELEASE_AUTHORIZATION_SCHEMA_ID, RELEASE_AUTHORIZATION_SCHEMA_VERSION,
    RELEASE_AUTHORIZATION_SELECTION, RELEASE_AUTHORIZATION_STABLE_CHANNEL,
    RELEASE_OPERATION_ASSET_SELECTION, RELEASE_OPERATION_IDENTITY_SCHEMA_ID,
    ReleaseAuthorizationAuthorityKindV1, ReleaseAuthorizationAuthorityV1,
    ReleaseAuthorizationConsumptionV1, ReleaseAuthorizationEvidenceV1,
    ReleaseAuthorizationExpectedContextV1, ReleaseAuthorizationFreezeV1,
    ReleaseAuthorizationInputV1, ReleaseAuthorizationOperationV1, ReleaseAuthorizationPackageRowV1,
    ReleaseAuthorizationResultV1, ReleaseAuthorizationSecretAvailabilityV1,
    ReleaseAuthorizationSecretStateV1, ReleaseAuthorizationSharedRowV1,
    ReleaseAuthorizationSourceKindV1, ReleaseAuthorizationSourceV1,
    ReleaseAuthorizationUseObservationV1, TrustClassV1, UntrustedInputPostureV1,
    acquire_operation_lease_for_operation_v1, append_release_operation_event_v1,
    authorization_evidence_digest_v1, build_release_operation_identity_v1,
    compile_release_authorization_v1, compile_release_operation_head_v1,
    mint_authorization_custody_v1, note_custody_readback_v1, operation_lease_subject_digest_v1,
    release_authorization_denominator_binding_v1, release_operation_identity_digest_v1,
    renew_operation_lease_v1, select_authorization_for_operation_v1,
    validate_release_operation_head_v1, validate_release_operation_history_v1,
    validate_release_operation_identity_v1, verify_custody_readback_v1, verify_lease_readback_v1,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const WORKFLOW: &str = ".github/workflows/release.yml";
const NOW: u64 = 1_786_000_200;
const HEAD_AT: u64 = NOW - 40;
const NONCE: &str = "synthetic-provider-roundtrip-authorization";
const SOURCE_BODY: &[u8] =
    b"Authorize publish_cargo_allow_final_0_2_0 for v0.2.0.\nSynthetic exact freeze source.\n";

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn byte_digest(bytes: &[u8]) -> String {
    allow_core::sha256_v1_bytes(bytes).replacen("sha256:v1:", "sha256:", 1)
}

fn require(ok: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

fn json_bytes<T: Serialize>(record: &T) -> Result<Vec<u8>, Box<dyn Error>> {
    // Transport must preserve even the insignificant whitespace, not merely
    // parse and serialize an equivalent record.
    let mut bytes = serde_json::to_vec_pretty(record)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn read_record<T: DeserializeOwned>(root: &Path, name: &str) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(root.join(name))?)?)
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::other("repository root is missing").into())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, Box<dyn Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-operation-store-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_python(arguments: &[&OsStr]) -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    for executable in ["python3", "python"] {
        let mut command = Command::new(executable);
        command
            .env_clear()
            .current_dir(&root)
            .arg("-B")
            .arg(root.join("scripts/test-release-operation-store.py"))
            .args(arguments);
        // A closed child environment preserves executable/temp discovery
        // without passing Git overrides, credentials, proxies or scan config.
        for key in ["PATH", "SystemRoot", "WINDIR", "TMP", "TEMP"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        match command.output() {
            Ok(output) => {
                return require(
                    output.status.success(),
                    format!(
                        "intercepted Python adapter test failed:\n{}\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    ),
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::other("Python 3 is required for provider adapter tests").into())
}

fn expected_context() -> Result<ReleaseAuthorizationExpectedContextV1, Box<dyn Error>> {
    let mut packages = Vec::new();
    let mut shared_prerequisites = Vec::new();
    for (index, (logical, package, version, shared)) in
        RELEASE_AUTHORIZATION_SELECTION.into_iter().enumerate()
    {
        if shared {
            shared_prerequisites.push(ReleaseAuthorizationSharedRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                expected_checksum: digest(100 + index as u64),
                authority_digest: digest(200 + index as u64),
            });
        } else {
            packages.push(ReleaseAuthorizationPackageRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                package_digest: digest(300 + index as u64),
                package_size_bytes: 10_000 + index as u64,
            });
        }
    }
    let mut freeze = ReleaseAuthorizationFreezeV1 {
        receipt_digest: digest(1),
        candidate_digest: digest(2),
        denominator_digest: String::new(),
        commit: "a".repeat(40),
        tree: "b".repeat(40),
        lock_digest: digest(3),
        topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
        packages,
        shared_prerequisites,
    };
    freeze.denominator_digest = release_authorization_denominator_binding_v1(&freeze)?;
    Ok(ReleaseAuthorizationExpectedContextV1 {
        schema_id: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_VERSION,
        repository: REPOSITORY.to_string(),
        freeze,
        evidence: ReleaseAuthorizationEvidenceV1 {
            package_docs_digest: digest(30),
            preflight_result: FinalRegistryPreflightResultV1::Complete,
            preflight_evaluated_at_unix_seconds: NOW - 120,
            preflight_maximum_age_seconds: 180,
            support_digest: digest(31),
            manifest_digest: digest(32),
            rehearsal_complete_except_authorization: true,
            rehearsal_digest: digest(33),
            source_controls_digest: digest(34),
            live_controls_digest: digest(35),
            workflow_digest: digest(36),
            action_inventory_digest: digest(37),
            observed_context_digest: digest(38),
            current_context_digest: digest(38),
        },
        secret_availability: ReleaseAuthorizationSecretAvailabilityV1 {
            redacted: true,
            state: ReleaseAuthorizationSecretStateV1::Available,
        },
        use_observation: ReleaseAuthorizationUseObservationV1 {
            state: ReleaseAuthorizationConsumptionV1::Available,
            consumed_nonces: Vec::new(),
        },
        frozen_file_digests: vec![digest(900)],
        evaluated_at_unix_seconds: NOW,
    })
}

fn decision(expected: &ReleaseAuthorizationExpectedContextV1) -> ReleaseAuthorizationInputV1 {
    // The expected fixture is selected before constructing this decision and
    // remains outside the downloaded archive. This is not a live evidence
    // assembler or an assertion that the fixture source authorized a release.
    ReleaseAuthorizationInputV1 {
        schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_SCHEMA_VERSION,
        operation: ReleaseAuthorizationOperationV1 {
            name: RELEASE_AUTHORIZATION_FINAL_OPERATION.to_string(),
            version: RELEASE_AUTHORIZATION_FINAL_VERSION.to_string(),
            tag: RELEASE_AUTHORIZATION_FINAL_TAG.to_string(),
            channel: RELEASE_AUTHORIZATION_STABLE_CHANNEL.to_string(),
            github_prerelease: false,
            authority_kind: ReleaseAuthorizationAuthorityKindV1::Clean,
        },
        freeze: expected.freeze.clone(),
        evidence: expected.evidence.clone(),
        authority: ReleaseAuthorizationAuthorityV1 {
            selected_auth_class: RELEASE_AUTHORIZATION_AUTH_CLASS.to_string(),
            maintainer_actor: "release-operator".to_string(),
            maintainer_role: "release-maintainer".to_string(),
            source: ReleaseAuthorizationSourceV1 {
                kind: ReleaseAuthorizationSourceKindV1::IssueComment,
                repository: REPOSITORY.to_string(),
                reference: "issue:3760#comment:202".to_string(),
                author: "release-operator".to_string(),
                body_digest: byte_digest(SOURCE_BODY),
                statement: RELEASE_AUTHORIZATION_EXACT_STATEMENT.to_string(),
            },
            created_at_unix_seconds: NOW - 100,
            expires_at_unix_seconds: NOW + 3600,
            one_run_scope: true,
            nonce: NONCE.to_string(),
        },
    }
}

fn operation(
    expected: &ReleaseAuthorizationExpectedContextV1,
    authorization_digest: &str,
) -> Result<CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
    let packages = expected
        .freeze
        .packages
        .iter()
        .map(|row| CargoAllowReleaseOperationPackageRowV1 {
            logical_id: row.logical_id.clone(),
            package_name: row.package_name.clone(),
            package_version: row.package_version.clone(),
            package_digest: row.package_digest.clone(),
        })
        .collect();
    let assets = RELEASE_OPERATION_ASSET_SELECTION
        .iter()
        .enumerate()
        .map(
            |(index, (asset_id, asset_name))| CargoAllowReleaseOperationAssetRowV1 {
                asset_id: (*asset_id).to_string(),
                asset_name: (*asset_name).to_string(),
                asset_digest: digest(500 + index as u64),
            },
        )
        .collect();
    Ok(
        build_release_operation_identity_v1(CargoAllowReleaseOperationIdentityInitV1 {
            nonce: "synthetic-provider-roundtrip-operation".to_string(),
            operation_class: CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            authority_kind: CargoAllowReleaseOperationAuthorityKindV1::Clean,
            repository: REPOSITORY.to_string(),
            product: "cargo-allow".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            github_prerelease: false,
            freeze_digest: expected.freeze.receipt_digest.clone(),
            final_evidence_graph_digest: digest(41),
            custody_digest: digest(42),
            replay_digest: digest(43),
            authorization_digest: authorization_digest.to_string(),
            cargo_lock_digest: expected.freeze.lock_digest.clone(),
            topology_digest: digest(44),
            support_digest: expected.evidence.support_digest.clone(),
            channel_digest: digest(45),
            packages,
            assets,
            workflow_digest: expected.evidence.workflow_digest.clone(),
            action_inventory_digest: expected.evidence.action_inventory_digest.clone(),
            live_controls_digest: expected.evidence.live_controls_digest.clone(),
            incident_predecessor_operation_digest: None,
            incident_predecessor_head_digest: None,
            one_run_scope: true,
            expires_at_unix_seconds: NOW + 3600,
        })
        .map_err(io::Error::other)?,
    )
}

fn producer() -> ProducerIdentityV1 {
    ProducerIdentityV1 {
        repository: REPOSITORY.to_string(),
        workflow_path: WORKFLOW.to_string(),
        git_ref: "refs/heads/main".to_string(),
        run_id: 101,
        run_attempt: 1,
        job_id: "303".to_string(),
        commit_sha: "a".repeat(40),
        tree_sha: "b".repeat(40),
        release_version: "0.2.0".to_string(),
        tool_name: "cargo-allow".to_string(),
        schema_id: RELEASE_OPERATION_IDENTITY_SCHEMA_ID.to_string(),
        producer_generation: 1,
    }
}

fn history(
    identity: &CargoAllowReleaseOperationIdentityV1,
    lease: &CargoAllowReleaseOperationLeaseV1,
) -> Result<Vec<CargoAllowReleaseOperationEventV1>, Box<dyn Error>> {
    let mut events = Vec::new();
    for (event_class, payload_schema_id, payload_digest, observed_at_unix_seconds) in [
        (
            CargoAllowReleaseOperationEventClassV1::OperationSelected,
            RELEASE_OPERATION_IDENTITY_SCHEMA_ID,
            release_operation_identity_digest_v1(identity)?,
            NOW - 70,
        ),
        (
            CargoAllowReleaseOperationEventClassV1::AuthorizationSelected,
            RELEASE_AUTHORIZATION_SCHEMA_ID,
            identity.authorization_digest.clone(),
            NOW - 59,
        ),
        (
            CargoAllowReleaseOperationEventClassV1::LeaseAcquired,
            OPERATION_LEASE_SCHEMA_ID,
            byte_digest(&json_bytes(lease)?),
            NOW - 49,
        ),
    ] {
        let event = append_release_operation_event_v1(
            identity,
            &events,
            CargoAllowReleaseOperationEventInitV1 {
                event_class,
                subject: CargoAllowReleaseOperationEventSubjectV1::Operation,
                payload_schema_id: payload_schema_id.to_string(),
                payload_digest,
                producer: CargoAllowReleaseOperationProducerV1 {
                    tool: "cargo-allow".to_string(),
                    schema: "cargo-allow.release-operation-producer.v1".to_string(),
                    generation: 1,
                    repository: REPOSITORY.to_string(),
                    workflow: WORKFLOW.to_string(),
                    workflow_ref: "refs/heads/main".to_string(),
                    run: "101".to_string(),
                    attempt: 1,
                    job: "303".to_string(),
                    commit: "a".repeat(40),
                },
                actor: "release-operator".to_string(),
                authority_class: CargoAllowReleaseOperationAuthorityKindV1::Clean,
                request_boundary: "synthetic-provider-roundtrip".to_string(),
                response_posture: CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
                semantic_result: CargoAllowReleaseOperationSemanticResultV1::Exact,
                artifact_digest: None,
                timestamp_source: CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
                observed_at_unix_seconds,
            },
        )
        .map_err(io::Error::other)?;
        events.push(event);
    }
    Ok(events)
}

fn write_files(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<(), Box<dyn Error>> {
    fs::create_dir(root)?;
    for (name, bytes) in files {
        fs::write(root.join(name), bytes)?;
    }
    Ok(())
}

#[test]
fn release_operation_store_protocol_controls() -> Result<(), Box<dyn Error>> {
    run_python(&[OsStr::new("-q")])
}

#[test]
fn release_operation_store_preserves_existing_domain_bytes() -> Result<(), Box<dyn Error>> {
    let expected = expected_context()?;
    let expected_bytes = json_bytes(&expected)?;
    let decision = decision(&expected);
    let compiled = compile_release_authorization_v1(&decision, &expected_bytes);
    require(
        compiled.result == ReleaseAuthorizationResultV1::Complete,
        format!(
            "synthetic authorization fixture differs: {:?}",
            compiled.findings
        ),
    )?;
    let identity = operation(&expected, &compiled.authorization_digest)?;
    let operation_digest = release_operation_identity_digest_v1(&identity)?;
    let lease_key = OperationLeaseKeyV1 {
        operation: OPERATION_LEASE_FINAL_OPERATION.to_string(),
        operation_identity_digest: operation_digest.clone(),
        version: OPERATION_LEASE_FINAL_VERSION.to_string(),
        tag: OPERATION_LEASE_FINAL_TAG.to_string(),
        commit: expected.freeze.commit.clone(),
        tree: expected.freeze.tree.clone(),
        denominator_digest: expected.freeze.denominator_digest.clone(),
    };
    let subject_digest = operation_lease_subject_digest_v1(&lease_key)?;
    let subject = subject_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| io::Error::other("lease subject digest is malformed"))?;
    let minted = mint_authorization_custody_v1(AuthorizationCustodyMintInitV1 {
        authorization_id: "synthetic-provider-authorization".to_string(),
        decision: decision.clone(),
        freeze_receipt_digest: expected.freeze.receipt_digest.clone(),
        replay_digest: identity.replay_digest.clone(),
        replay_result: "Complete".to_string(),
        candidate_custody_digest: identity.custody_digest.clone(),
        freeze_complete: true,
        replay_complete: true,
        // A logical control ref is knowable before upload. The finalized
        // numeric artifact ID belongs only to the external transfer envelope.
        storage_locator: format!(
            "https://api.github.com/repos/{REPOSITORY}/git/ref/heads/cargo-allow-release-control/{subject}"
        ),
        repository_root: String::new(),
        storage_access_policy: "synthetic-selected-release-store".to_string(),
        storage_retention_expiry_unix_seconds: NOW + 86_400,
        storage_provider_available: true,
        valid_from_unix_seconds: NOW - 100,
        expires_at_unix_seconds: NOW + 3600,
        minted_by: "maintainer:release-operator".to_string(),
        minted_at_unix_seconds: NOW - 80,
    })
    .map_err(io::Error::other)?;
    let minted_bytes = json_bytes(&minted)?;
    let mut custody = minted.clone();
    require(
        note_custody_readback_v1(&mut custody, &minted_bytes) == CustodyReadbackV1::Match,
        "existing custody reducer rejected its fixture bytes",
    )?;
    let consumption = select_authorization_for_operation_v1(
        &identity,
        &mut custody,
        NONCE,
        NOW - 60,
        &authorization_evidence_digest_v1(&decision.evidence)?,
        true,
    )
    .map_err(io::Error::other)?;
    let lease = acquire_operation_lease_for_operation_v1(
        &identity,
        OperationLeaseAcquireInitV1 {
            lease_id: "synthetic-provider-lease".to_string(),
            class: OperationLeaseClassV1::Clean,
            key: lease_key,
            holder_workflow: WORKFLOW.to_string(),
            holder_run: "101".to_string(),
            holder_attempt: "1".to_string(),
            holder_job: "303".to_string(),
            // These fixed synthetic heads remain fixed during renewal. There
            // is no checked journal/checkpoint rebind reducer in this slice.
            journal_head_digest: digest(700),
            checkpoint_head_digest: digest(701),
            max_renewals: 3,
            acquired_at_unix_seconds: NOW - 50,
            expires_at_unix_seconds: NOW + 100,
            storage_provider_available: true,
        },
        None,
        NOW - 50,
    )
    .map_err(io::Error::other)?;
    require(
        lease.subject_digest == subject_digest,
        "existing lease subject differs from its preselected logical locator",
    )?;
    let events = history(&identity, &lease)?;
    let head =
        compile_release_operation_head_v1(&identity, &events, HEAD_AT).map_err(io::Error::other)?;
    let before = BTreeMap::from([
        ("authorization.json".to_string(), json_bytes(&decision)?),
        ("custody-mint.json".to_string(), minted_bytes),
        ("custody.json".to_string(), json_bytes(&custody)?),
        ("consumption.json".to_string(), json_bytes(&consumption)?),
        ("identity.json".to_string(), json_bytes(&identity)?),
        ("events.json".to_string(), json_bytes(&events)?),
        ("head.json".to_string(), json_bytes(&head)?),
        ("lease.json".to_string(), json_bytes(&lease)?),
    ]);
    let mut renewed = lease.clone();
    renew_operation_lease_v1(&mut renewed, NOW, NOW + 200, true).map_err(io::Error::other)?;
    let mut after = before.clone();
    after.insert("lease.json".to_string(), json_bytes(&renewed)?);
    let producer = producer();
    let transfer = CargoAllowReleaseArtifactTransferV1::new(ArtifactTransferInitV1 {
        transfer_id: "synthetic-provider-transfer-505".to_string(),
        role: "release-operation-control".to_string(),
        stable_artifact_id: "505".to_string(),
        producer: producer.clone(),
        provider_id: "github-actions-artifact".to_string(),
        provider_artifact_name: "selected-release-authorization".to_string(),
        files: before
            .iter()
            .map(|(name, bytes)| ArtifactTransferFileV1 {
                path: name.clone(),
                size_bytes: bytes.len() as u64,
                sha256: byte_digest(bytes),
            })
            .collect(),
        semantic_payload_digest: Some(operation_digest.clone()),
        trust_class: TrustClassV1::ManualDispatch,
        untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
        created_at_utc: "2026-08-06T07:09:50Z".to_string(),
    });
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input");
    let output = scratch.0.join("output");
    fs::create_dir(&input)?;
    write_files(&input.join("before"), &before)?;
    write_files(&input.join("after"), &after)?;
    // The finalized artifact-ID envelope stays outside its own payload.
    fs::write(input.join("transfer.json"), json_bytes(&transfer)?)?;
    fs::write(input.join("producer.json"), json_bytes(&producer)?)?;
    fs::write(
        input.join("source.json"),
        json_bytes(&decision.authority.source)?,
    )?;
    fs::write(input.join("source.body"), SOURCE_BODY)?;
    fs::write(input.join("subject.txt"), &lease.subject_digest)?;
    fs::write(input.join("operation.txt"), &operation_digest)?;
    run_python(&[
        OsStr::new("--typed-input"),
        input.as_os_str(),
        OsStr::new("--typed-output"),
        output.as_os_str(),
    ])?;
    for (name, expected_bytes) in &after {
        require(
            fs::read(output.join(name))? == *expected_bytes,
            format!("Git storage changed exact bytes: {name}"),
        )?;
    }
    let mut downloaded_files = Vec::new();
    for (name, expected_bytes) in &before {
        let downloaded = fs::read(output.join("downloaded").join(name))?;
        require(
            downloaded == *expected_bytes,
            format!("artifact download changed exact bytes: {name}"),
        )?;
        downloaded_files.push(ActualDownloadedFileV1 {
            path: name.clone(),
            size_bytes: downloaded.len() as u64,
            sha256: byte_digest(&downloaded),
        });
    }
    require(
        transfer.evaluate_transfer(
            &ConsumerContextV1 {
                workflow_path: WORKFLOW.to_string(),
                run_id: 101,
                job_id: "303".to_string(),
                requested_role: "release-operation-control".to_string(),
                is_credential_bearing: true,
            },
            &producer.commit_sha,
            "0.2.0",
            &downloaded_files,
        ) == ArtifactTransferDispositionV1::Complete,
        "existing transfer evaluator rejected actual downloaded bytes",
    )?;
    let returned_source = fs::read(output.join("source.body"))?;
    require(
        returned_source == SOURCE_BODY
            && byte_digest(&returned_source) == decision.authority.source.body_digest,
        "authenticated source readback changed exact UTF-8 bytes",
    )?;
    let returned_decision: ReleaseAuthorizationInputV1 =
        read_record(&output, "authorization.json")?;
    require(
        compile_release_authorization_v1(&returned_decision, &expected_bytes).result
            == ReleaseAuthorizationResultV1::Complete,
        "existing authorization compiler rejected returned decision bytes",
    )?;
    let returned_identity: CargoAllowReleaseOperationIdentityV1 =
        read_record(&output, "identity.json")?;
    let returned_events: Vec<CargoAllowReleaseOperationEventV1> =
        read_record(&output, "events.json")?;
    let returned_head: CargoAllowReleaseOperationHeadV1 = read_record(&output, "head.json")?;
    validate_release_operation_identity_v1(&returned_identity).map_err(io::Error::other)?;
    validate_release_operation_history_v1(&returned_identity, &returned_events)
        .map_err(io::Error::other)?;
    validate_release_operation_head_v1(
        &returned_identity,
        &returned_events,
        HEAD_AT,
        &returned_head,
    )
    .map_err(io::Error::other)?;
    require(
        release_operation_identity_digest_v1(&returned_identity)? == operation_digest,
        "canonical operation identity changed through provider storage",
    )?;
    let returned_custody: CargoAllowReleaseAuthorizationCustodyV1 =
        read_record(&output, "custody.json")?;
    let returned_consumption: CargoAllowReleaseAuthorizationConsumptionV1 =
        read_record(&output, "consumption.json")?;
    require(
        returned_custody == custody && returned_consumption == consumption,
        "existing custody or selection observation changed through provider storage",
    )?;
    require(
        verify_custody_readback_v1(&minted, &fs::read(output.join("custody-mint.json"))?)
            == CustodyReadbackV1::Match,
        "immutable mint bytes no longer satisfy the existing custody readback",
    )?;
    let lease_bytes = fs::read(output.join("lease.json"))?;
    let returned_lease: CargoAllowReleaseOperationLeaseV1 = serde_json::from_slice(&lease_bytes)?;
    require(
        returned_lease == renewed
            && verify_lease_readback_v1(&renewed, &lease_bytes) == LeaseReadbackV1::Match
            && returned_lease.journal_head_digest == lease.journal_head_digest
            && returned_lease.checkpoint_head_digest == lease.checkpoint_head_digest,
        "existing lease renewal or fixed head bindings changed during storage",
    )
}
