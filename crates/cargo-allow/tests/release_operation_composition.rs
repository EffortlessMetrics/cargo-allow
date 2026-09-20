//! Canonical operation composition across release children (#3940 PR C).
//!
//! Synthetic subjects only: no network access, no uploads, no provider API
//! calls, no credentials, and nothing leaves the process. This test proves
//! the PR B composition claim: one exact canonical operation identity and
//! head flows through the publication journal, the remote checkpoint, the
//! lease, and the tag transaction. Provider-specific contracts own their
//! payloads; none of them invents operation identity, current head, sequence
//! law, or terminal meaning.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use allow_report::{
    CargoAllowPublicationJournalV1, CargoAllowReleaseOperationAssetRowV1,
    CargoAllowReleaseOperationAuthorityKindV1, CargoAllowReleaseOperationClassV1,
    CargoAllowReleaseOperationEventClassV1, CargoAllowReleaseOperationEventInitV1,
    CargoAllowReleaseOperationEventSubjectV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationIdentityV1, CargoAllowReleaseOperationPackageRowV1,
    CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
    CargoAllowReleaseOperationTimestampSourceV1, CheckpointProviderOutcomeV1,
    FINAL_TAG_REQUIRED_AUTHORIZATION_STATE, FINAL_TAG_REQUIRED_LEASE_STATE, FinalTagIdentityV1,
    FinalTagRemoteObservationV1, FinalTagTransactionInitV1, OPERATION_LEASE_FINAL_OPERATION,
    OPERATION_LEASE_FINAL_TAG, OPERATION_LEASE_FINAL_VERSION, OperationLeaseAcquireInitV1,
    OperationLeaseClassV1, OperationLeaseKeyV1, PublicationCheckpointKindV1,
    PublicationCheckpointProducerV1, PublicationCheckpointProviderObjectV1,
    PublicationCheckpointProviderV1, PublicationCheckpointRowStateV1, PublicationCheckpointRowV1,
    PublicationJournalAppendV1, PublicationJournalClassV1, PublicationJournalEventV1,
    PublicationJournalInitV1, PublicationJournalRowV1, RELEASE_AUTHORIZATION_SELECTION,
    RELEASE_OPERATION_ASSET_SELECTION, acquire_operation_lease_for_operation_v1,
    append_journal_event_v1, append_release_operation_event_v1,
    begin_publication_checkpoint_for_operation_v1, begin_publication_journal_for_operation_v1,
    begin_tag_transaction_for_operation_v1, build_release_operation_identity_v1,
    compile_release_operation_head_v1, digest_publication_checkpoint_body_v1,
    record_checkpoint_readback_with_witness_v1, release_operation_head_digest_v1,
    release_operation_identity_digest_v1, render_publication_checkpoint_v1,
    render_publication_journal_v1, verify_checkpoint_against_journal_v1,
};

const CREATED_AT: u64 = 1_786_200_000;
const NOW: u64 = 1_786_200_500;
const EVALUATED_AT: u64 = 1_790_100_000;

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

fn fail<T>(message: impl Into<String>) -> Result<T, Box<dyn Error>> {
    Err(io::Error::other(message.into()).into())
}

fn canonical_identity(nonce: &str) -> Result<CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
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

fn canonical_head(
    identity: &CargoAllowReleaseOperationIdentityV1,
) -> Result<allow_report::CargoAllowReleaseOperationHeadV1, Box<dyn Error>> {
    let producer = allow_report::CargoAllowReleaseOperationProducerV1 {
        tool: "cargo-allow".to_string(),
        schema: "cargo-allow.release-operation-producer.v1".to_string(),
        generation: 1,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow: "release".to_string(),
        workflow_ref: "refs/heads/main".to_string(),
        run: "4242".to_string(),
        attempt: 1,
        job: "composition".to_string(),
        commit: "a".repeat(40),
    };
    let init = |class: CargoAllowReleaseOperationEventClassV1, ordinal: u64| {
        let payload_digest =
            if class == CargoAllowReleaseOperationEventClassV1::AuthorizationSelected {
                identity.authorization_digest.clone()
            } else {
                digest(1_000 + ordinal)
            };
        CargoAllowReleaseOperationEventInitV1 {
            event_class: class,
            subject: CargoAllowReleaseOperationEventSubjectV1::Operation,
            payload_schema_id: "cargo-allow.synthetic-release-payload.v1".to_string(),
            payload_digest,
            producer: producer.clone(),
            actor: "release-operator".to_string(),
            authority_class: identity.authority_kind,
            request_boundary: "synthetic-no-provider-call".to_string(),
            response_posture: CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
            semantic_result: CargoAllowReleaseOperationSemanticResultV1::Exact,
            artifact_digest: Some(digest(2_000 + ordinal)),
            observed_at_unix_seconds: 1_790_000_000 + ordinal,
            timestamp_source: CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
        }
    };
    let mut events = Vec::new();
    for (class, ordinal) in [
        (CargoAllowReleaseOperationEventClassV1::OperationSelected, 1),
        (
            CargoAllowReleaseOperationEventClassV1::AuthorizationSelected,
            2,
        ),
        (CargoAllowReleaseOperationEventClassV1::LeaseAcquired, 3),
    ] {
        let event = append_release_operation_event_v1(identity, &events, init(class, ordinal))
            .map_err(io::Error::other)?;
        events.push(event);
    }
    Ok(
        compile_release_operation_head_v1(identity, &events, EVALUATED_AT)
            .map_err(io::Error::other)?,
    )
}

fn journal_row() -> PublicationJournalRowV1 {
    PublicationJournalRowV1 {
        package_name: "cargo-allow".to_string(),
        version: "0.2.0".to_string(),
        row_order: 0,
        candidate_archive_digest: digest(10),
        depends_on: Vec::new(),
    }
}

fn settled_journal(
    identity: &CargoAllowReleaseOperationIdentityV1,
) -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    let mut journal = begin_publication_journal_for_operation_v1(
        identity,
        PublicationJournalInitV1 {
            journal_id: "journal-0-2-0-001".to_string(),
            operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
            operation_identity_digest: digest(0),
            operation_class: PublicationJournalClassV1::CleanFinalPublication,
            authorization_digest: identity.authorization_digest.clone(),
            custody_digest: identity.custody_digest.clone(),
            freeze_digest: identity.freeze_digest.clone(),
            prior_journal_digest: None,
            rows: vec![journal_row()],
            created_at_unix_seconds: CREATED_AT,
            workflow: "release".to_string(),
            run: "4242".to_string(),
            attempt: "1".to_string(),
            job: "publish".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    let row = journal_row();
    let mut at = CREATED_AT;
    let mut append = |kind: PublicationJournalEventV1, row: Option<PublicationJournalRowV1>| {
        at += 10;
        append_journal_event_v1(
            &mut journal,
            PublicationJournalAppendV1 {
                kind,
                row,
                response: None,
                observation: None,
                at_unix_seconds: at,
                reason: "synthetic".to_string(),
            },
        )
        .map_err(io::Error::other)
    };
    append(PublicationJournalEventV1::OperationSelected, None)?;
    append(PublicationJournalEventV1::AuthorizationConsumed, None)?;
    append(PublicationJournalEventV1::TagObservedExact, None)?;
    append(
        PublicationJournalEventV1::RowPreflightComplete,
        Some(row.clone()),
    )?;
    append(PublicationJournalEventV1::UploadIntentDurable, Some(row))?;
    Ok(journal)
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    Ok(crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?
        .to_path_buf())
}

fn checkpoint_producer() -> PublicationCheckpointProducerV1 {
    PublicationCheckpointProducerV1 {
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "publish".to_string(),
        git_ref: "refs/tags/v0.2.0".to_string(),
        commit: "4d1b0fe10e82f506424f9dc7b7ff3bf81403191b".to_string(),
    }
}

fn store_checkpoint(
    checkpoint: &mut allow_report::CargoAllowPublicationCheckpointV1,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let body_digest =
        digest_publication_checkpoint_body_v1(checkpoint).map_err(io::Error::other)?;
    checkpoint.provider.object_digest = body_digest;
    for _ in 0..4 {
        let bytes = render_publication_checkpoint_v1(checkpoint)
            .map_err(io::Error::other)?
            .into_bytes();
        let len = bytes.len() as u64;
        if checkpoint.provider.object_size_bytes == len {
            return Ok(bytes);
        }
        checkpoint.provider.object_size_bytes = len;
    }
    fail("checkpoint stored size must converge")
}

#[test]
fn one_canonical_operation_across_journal_checkpoint_lease_and_tag() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("composition-0001")?;
    let head = canonical_head(&identity)?;
    let identity_digest =
        release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?;
    let head_digest = release_operation_head_digest_v1(&head).map_err(io::Error::other)?;

    // Journal settles a prefix under the canonical identity.
    let journal = settled_journal(&identity)?;
    require(
        journal.operation_identity_digest == identity_digest,
        "journal must carry the canonical identity digest",
    )?;
    let journal_head = journal
        .entries
        .last()
        .ok_or_else(|| io::Error::other("settled journal must have a head"))?;

    // Checkpoint gates the row under the same identity and head, then reads
    // back Complete against the live journal.
    let mut checkpoint = begin_publication_checkpoint_for_operation_v1(
        &identity,
        &head,
        allow_report::PublicationCheckpointInitV1 {
            checkpoint_id: "checkpoint-0-2-0-001".to_string(),
            operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
            operation_identity_digest: digest(0),
            operation_head_digest: digest(0),
            operation_class: allow_report::PublicationCheckpointClassV1::CleanFinalPublication,
            authorization_digest: identity.authorization_digest.clone(),
            custody_digest: identity.custody_digest.clone(),
            freeze_digest: identity.freeze_digest.clone(),
            journal_head_sequence: journal_head.sequence,
            journal_head_digest: journal_head.entry_digest.clone(),
            checkpoint_sequence: 1,
            kind: PublicationCheckpointKindV1::PreIntentDurable,
            row: PublicationCheckpointRowV1 {
                package_name: "cargo-allow".to_string(),
                row_order: 0,
                state: PublicationCheckpointRowStateV1::IntentDurable,
            },
            first_irreversible_row: None,
            incident_recorded: false,
            provider: PublicationCheckpointProviderObjectV1 {
                provider: PublicationCheckpointProviderV1::GithubActionsArtifact,
                object_id: "artifact-1".to_string(),
                object_name: "publication-checkpoint".to_string(),
                object_digest: digest(0),
                object_size_bytes: 1,
            },
            producer: checkpoint_producer(),
            retention_days: 30,
            created_at_unix_seconds: CREATED_AT,
            note: "synthetic".to_string(),
        },
        None,
    )
    .map_err(io::Error::other)?;
    require(
        checkpoint.operation_identity_digest == identity_digest
            && checkpoint.operation_head_digest == head_digest,
        "checkpoint must carry the canonical identity and head digests",
    )?;
    let stored = store_checkpoint(&mut checkpoint)?;
    let (_, witness) = record_checkpoint_readback_with_witness_v1(
        &mut checkpoint,
        CheckpointProviderOutcomeV1::Delivered(stored),
        NOW,
    )
    .map_err(io::Error::other)?;
    let witness =
        witness.ok_or_else(|| io::Error::other("Complete readback must return a witness"))?;
    verify_checkpoint_against_journal_v1(
        &checkpoint,
        &witness,
        &journal,
        &checkpoint_producer(),
        NOW,
    )
    .map_err(io::Error::other)?;

    // Lease acquires under the same identity, bound to the live heads.
    let lease = acquire_operation_lease_for_operation_v1(
        &identity,
        OperationLeaseAcquireInitV1 {
            lease_id: "lease-0-2-0-001".to_string(),
            class: OperationLeaseClassV1::Clean,
            key: OperationLeaseKeyV1 {
                operation: OPERATION_LEASE_FINAL_OPERATION.to_string(),
                operation_identity_digest: digest(0),
                version: OPERATION_LEASE_FINAL_VERSION.to_string(),
                tag: OPERATION_LEASE_FINAL_TAG.to_string(),
                commit: "c".repeat(40),
                tree: "d".repeat(40),
                denominator_digest: digest(7),
            },
            holder_workflow: "release.yml".to_string(),
            holder_run: "101".to_string(),
            holder_attempt: "1".to_string(),
            holder_job: "publish".to_string(),
            journal_head_digest: journal_head.entry_digest.clone(),
            checkpoint_head_digest: allow_report::digest_publication_checkpoint_link_v1(
                &checkpoint,
            )
            .map_err(io::Error::other)?,
            max_renewals: 3,
            acquired_at_unix_seconds: CREATED_AT,
            expires_at_unix_seconds: CREATED_AT + 3600,
            storage_provider_available: true,
        },
        None,
        NOW,
    )
    .map_err(io::Error::other)?;
    require(
        lease.key.operation_identity_digest == identity_digest,
        "lease key must carry the canonical identity digest",
    )?;

    // Tag commits to the same identity plus the exact lease key digest: the
    // tag cannot name a lease from another operation.
    let tag = begin_tag_transaction_for_operation_v1(
        &identity,
        FinalTagTransactionInitV1 {
            transaction_id: "tag-tx-0-2-0-001".to_string(),
            operation_identity_digest: digest(0),
            authorization_digest: identity.authorization_digest.clone(),
            authorization_observed_state: FINAL_TAG_REQUIRED_AUTHORIZATION_STATE.to_string(),
            lease_key_digest: allow_report::operation_lease_key_digest_v1(&lease.key)
                .map_err(io::Error::other)?,
            lease_observed_state: FINAL_TAG_REQUIRED_LEASE_STATE.to_string(),
            lease_holder_generation: 1,
            custody_commit: "a".repeat(40),
            custody_tree: "b".repeat(40),
            freeze_digest: identity.freeze_digest.clone(),
            custody_digest: identity.custody_digest.clone(),
            replay_digest: identity.replay_digest.clone(),
            evidence_digest: digest(55),
            tag: FinalTagIdentityV1 {
                version: "0.2.0".to_string(),
                tag: "v0.2.0".to_string(),
                channel: "stable".to_string(),
                github_prerelease: false,
                commit: "a".repeat(40),
                tree: "b".repeat(40),
                tag_object_id: "c".repeat(40),
                tagger_digest: digest(60),
                message_digest: digest(61),
            },
            remote_repository: "EffortlessMetrics/cargo-allow".to_string(),
            remote_ref: "refs/tags/v0.2.0".to_string(),
            journal_prefix: "release-ops/0.2.0/tag".to_string(),
            workflow: "release.yml".to_string(),
            run: "101".to_string(),
            attempt: "1".to_string(),
            job: "tag".to_string(),
            remote_preflight: FinalTagRemoteObservationV1 {
                provider_reachable: true,
                ref_exists: false,
                remote_is_annotated: false,
                remote_object_id: String::new(),
                remote_peeled_commit: String::new(),
                remote_peeled_tree: String::new(),
            },
            created_at_unix_seconds: CREATED_AT,
        },
    )
    .map_err(io::Error::other)?;
    require(
        tag.operation_identity_digest == identity_digest
            && tag.lease_key_digest
                == allow_report::operation_lease_key_digest_v1(&lease.key)
                    .map_err(io::Error::other)?,
        "tag must name the canonical identity and the exact held lease key",
    )?;
    Ok(())
}

#[test]
fn foreign_operation_breaks_composition() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("composition-0001")?;
    let foreign = canonical_identity("composition-0002")?;
    let journal = settled_journal(&identity)?;

    // A checkpoint for another operation cannot gate this journal: the
    // journal cross-check pins the canonical identity digest.
    let foreign_head = canonical_head(&foreign)?;
    let journal_head = journal
        .entries
        .last()
        .ok_or_else(|| io::Error::other("settled journal must have a head"))?;
    let mut foreign_checkpoint = begin_publication_checkpoint_for_operation_v1(
        &foreign,
        &foreign_head,
        allow_report::PublicationCheckpointInitV1 {
            checkpoint_id: "checkpoint-foreign-001".to_string(),
            operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
            operation_identity_digest: digest(0),
            operation_head_digest: digest(0),
            operation_class: allow_report::PublicationCheckpointClassV1::CleanFinalPublication,
            authorization_digest: foreign.authorization_digest.clone(),
            custody_digest: digest(71),
            freeze_digest: digest(72),
            journal_head_sequence: journal_head.sequence,
            journal_head_digest: journal_head.entry_digest.clone(),
            checkpoint_sequence: 1,
            kind: PublicationCheckpointKindV1::PreIntentDurable,
            row: PublicationCheckpointRowV1 {
                package_name: "cargo-allow".to_string(),
                row_order: 0,
                state: PublicationCheckpointRowStateV1::IntentDurable,
            },
            first_irreversible_row: None,
            incident_recorded: false,
            provider: PublicationCheckpointProviderObjectV1 {
                provider: PublicationCheckpointProviderV1::GithubActionsArtifact,
                object_id: "artifact-9".to_string(),
                object_name: "publication-checkpoint".to_string(),
                object_digest: digest(0),
                object_size_bytes: 1,
            },
            producer: checkpoint_producer(),
            retention_days: 30,
            created_at_unix_seconds: CREATED_AT,
            note: "synthetic".to_string(),
        },
        None,
    )
    .map_err(io::Error::other)?;
    let stored = store_checkpoint(&mut foreign_checkpoint)?;
    let (_, witness) = record_checkpoint_readback_with_witness_v1(
        &mut foreign_checkpoint,
        CheckpointProviderOutcomeV1::Delivered(stored),
        NOW,
    )
    .map_err(io::Error::other)?;
    let witness =
        witness.ok_or_else(|| io::Error::other("Complete readback must return a witness"))?;
    require(
        verify_checkpoint_against_journal_v1(
            &foreign_checkpoint,
            &witness,
            &journal,
            &checkpoint_producer(),
            NOW,
        )
        .is_err(),
        "a foreign-operation checkpoint must never verify against this journal",
    )?;
    Ok(())
}

#[test]
fn composition_renderings_validate_against_schemas() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("composition-0001")?;
    let head = canonical_head(&identity)?;
    let journal = settled_journal(&identity)?;
    let journal_head = journal
        .entries
        .last()
        .ok_or_else(|| io::Error::other("settled journal must have a head"))?;
    let checkpoint = begin_publication_checkpoint_for_operation_v1(
        &identity,
        &head,
        allow_report::PublicationCheckpointInitV1 {
            checkpoint_id: "checkpoint-0-2-0-001".to_string(),
            operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
            operation_identity_digest: digest(0),
            operation_head_digest: digest(0),
            operation_class: allow_report::PublicationCheckpointClassV1::CleanFinalPublication,
            authorization_digest: identity.authorization_digest.clone(),
            custody_digest: identity.custody_digest.clone(),
            freeze_digest: identity.freeze_digest.clone(),
            journal_head_sequence: journal_head.sequence,
            journal_head_digest: journal_head.entry_digest.clone(),
            checkpoint_sequence: 1,
            kind: PublicationCheckpointKindV1::PreIntentDurable,
            row: PublicationCheckpointRowV1 {
                package_name: "cargo-allow".to_string(),
                row_order: 0,
                state: PublicationCheckpointRowStateV1::IntentDurable,
            },
            first_irreversible_row: None,
            incident_recorded: false,
            provider: PublicationCheckpointProviderObjectV1 {
                provider: PublicationCheckpointProviderV1::GithubActionsArtifact,
                object_id: "artifact-1".to_string(),
                object_name: "publication-checkpoint".to_string(),
                object_digest: digest(0),
                object_size_bytes: 1,
            },
            producer: checkpoint_producer(),
            retention_days: 30,
            created_at_unix_seconds: CREATED_AT,
            note: "synthetic".to_string(),
        },
        None,
    )
    .map_err(io::Error::other)?;
    let root = repository_root()?;
    if root.join(".git").exists() {
        for (name, rendered, schema_path) in [
            (
                "journal",
                render_publication_journal_v1(&journal)?,
                "docs/schemas/cargo-allow.publication-journal.v1.schema.json",
            ),
            (
                "checkpoint",
                render_publication_checkpoint_v1(&checkpoint)?,
                "docs/schemas/cargo-allow.publication-checkpoint.v1.schema.json",
            ),
        ] {
            let schema: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(root.join(schema_path))?)?;
            let validator = jsonschema::validator_for(&schema)
                .map_err(|error| io::Error::other(format!("{name} schema compiles: {error}")))?;
            let rendered: serde_json::Value = serde_json::from_str(&rendered)?;
            validator.validate(&rendered).map_err(|error| {
                io::Error::other(format!("composed {name} violates schema: {error}"))
            })?;
        }
    }
    Ok(())
}
