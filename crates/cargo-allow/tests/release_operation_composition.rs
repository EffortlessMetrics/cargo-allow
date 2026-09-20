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
    CargoAllowReleaseOperationEventSubjectV1, CargoAllowReleaseOperationEventV1,
    CargoAllowReleaseOperationHeadV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationIdentityV1, CargoAllowReleaseOperationPackageRowV1,
    CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
    CargoAllowReleaseOperationStateV1, CargoAllowReleaseOperationTimestampSourceV1,
    CheckpointProviderOutcomeV1, FINAL_TAG_REQUIRED_AUTHORIZATION_STATE,
    FINAL_TAG_REQUIRED_LEASE_STATE, FinalTagDurabilityV1, FinalTagIdentityV1,
    FinalTagRemoteObservationV1, FinalTagTransactionInitV1, OPERATION_LEASE_FINAL_OPERATION,
    OPERATION_LEASE_FINAL_TAG, OPERATION_LEASE_FINAL_VERSION, OperationLeaseAcquireInitV1,
    OperationLeaseClassV1, OperationLeaseKeyV1, PublicationCheckpointKindV1,
    PublicationCheckpointProducerV1, PublicationCheckpointProviderObjectV1,
    PublicationCheckpointProviderV1, PublicationCheckpointRowStateV1, PublicationCheckpointRowV1,
    PublicationJournalAppendV1, PublicationJournalClassV1, PublicationJournalEventV1,
    PublicationJournalInitV1, PublicationJournalRowV1, PublicationRegistryObservationV1,
    PublicationUploadResponseV1, RELEASE_AUTHORIZATION_SELECTION,
    RELEASE_OPERATION_ASSET_SELECTION, UploadResponseClassV1,
    acquire_operation_lease_for_operation_v1, append_journal_event_v1,
    append_release_operation_event_v1, begin_publication_checkpoint_for_operation_v1,
    begin_publication_journal_for_operation_v1, begin_tag_transaction_for_operation_v1,
    build_release_operation_identity_v1, compile_release_operation_head_v1,
    digest_publication_checkpoint_body_v1, digest_publication_checkpoint_link_v1,
    evaluate_release_operation_v1, note_lease_irreversible_start_v1,
    record_checkpoint_readback_with_witness_v1, record_tag_push_intent_v1,
    record_tag_push_response_v1, record_tag_push_started_v1, release_operation_head_digest_v1,
    release_operation_identity_digest_v1, render_publication_checkpoint_v1,
    render_publication_journal_v1, tag_push_intent_digest_v1, verify_checkpoint_against_journal_v1,
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

fn rehearsal_event_init(
    identity: &CargoAllowReleaseOperationIdentityV1,
    class: CargoAllowReleaseOperationEventClassV1,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    result: CargoAllowReleaseOperationSemanticResultV1,
    ordinal: u64,
) -> CargoAllowReleaseOperationEventInitV1 {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    let (artifact_digest, payload_digest, response_posture) = match &subject {
        CargoAllowReleaseOperationEventSubjectV1::Package(id) => (
            identity
                .packages
                .iter()
                .find(|row| row.logical_id == *id)
                .map(|row| row.package_digest.clone()),
            digest(1_000 + ordinal),
            CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        ),
        CargoAllowReleaseOperationEventSubjectV1::Asset(id) => (
            identity
                .assets
                .iter()
                .find(|row| row.asset_id == *id)
                .map(|row| row.asset_digest.clone()),
            digest(1_000 + ordinal),
            CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        ),
        CargoAllowReleaseOperationEventSubjectV1::Operation => (
            Some(digest(2_000 + ordinal)),
            if class == Event::AuthorizationSelected {
                identity.authorization_digest.clone()
            } else {
                digest(1_000 + ordinal)
            },
            CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        ),
    };
    let timestamp_source = match class {
        Event::TagObservedExact
        | Event::PackageRowObservedExact
        | Event::GitHubDraftObservedExact
        | Event::AssetObservedExact
        | Event::PublicReleaseObservedExact
        | Event::ContainmentObservedExact => {
            CargoAllowReleaseOperationTimestampSourceV1::ProviderMetadata
        }
        Event::RepositoryReconciled => {
            CargoAllowReleaseOperationTimestampSourceV1::RepositoryMetadata
        }
        _ => CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
    };
    CargoAllowReleaseOperationEventInitV1 {
        event_class: class,
        subject,
        payload_schema_id: "cargo-allow.synthetic-release-payload.v1".to_string(),
        payload_digest,
        producer: allow_report::CargoAllowReleaseOperationProducerV1 {
            tool: "cargo-allow".to_string(),
            schema: "cargo-allow.release-operation-producer.v1".to_string(),
            generation: 1,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            workflow: "release".to_string(),
            workflow_ref: "refs/heads/main".to_string(),
            run: "4242".to_string(),
            attempt: 1,
            job: "rehearsal".to_string(),
            commit: "a".repeat(40),
        },
        actor: "release-operator".to_string(),
        authority_class: identity.authority_kind,
        request_boundary: "synthetic-no-provider-call".to_string(),
        response_posture,
        semantic_result: result,
        artifact_digest,
        observed_at_unix_seconds: 1_790_000_000 + ordinal,
        timestamp_source,
    }
}

fn rehearsal_append(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    class: CargoAllowReleaseOperationEventClassV1,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    ordinal: u64,
) -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    let mut init = rehearsal_event_init(
        identity,
        class,
        subject.clone(),
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        ordinal,
    );
    if matches!(
        class,
        Event::TagObservedExact
            | Event::PackageRowObservedExact
            | Event::GitHubDraftObservedExact
            | Event::AssetObservedExact
            | Event::PublicReleaseObservedExact
            | Event::ContainmentObservedExact
    ) {
        init.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
        if let Some(request) = events.iter().rev().find(|event| {
            event.event_class == Event::IrreversibleRequestStarted && event.subject == subject
        }) {
            init.payload_schema_id = request.payload_schema_id.clone();
            init.payload_digest = request.payload_digest.clone();
            init.request_boundary = request.request_boundary.clone();
            init.artifact_digest = request.artifact_digest.clone();
        }
    }
    let event =
        append_release_operation_event_v1(identity, events, init).map_err(io::Error::other)?;
    events.push(event);
    Ok(())
}

fn rehearsal_append_request(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    ordinal: u64,
) -> Result<(), Box<dyn Error>> {
    let mut init = rehearsal_event_init(
        identity,
        CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted,
        subject,
        CargoAllowReleaseOperationSemanticResultV1::Unknown,
        ordinal,
    );
    init.payload_schema_id = format!("cargo-allow.synthetic-request-{ordinal}.v1");
    init.request_boundary = format!("synthetic-request-{ordinal}");
    init.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown;
    let event =
        append_release_operation_event_v1(identity, events, init).map_err(io::Error::other)?;
    events.push(event);
    Ok(())
}

fn rehearsal_journal(
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
    let mut append = |kind: PublicationJournalEventV1,
                      row: Option<PublicationJournalRowV1>,
                      with_response: bool,
                      with_observation: bool| {
        at += 10;
        append_journal_event_v1(
            &mut journal,
            PublicationJournalAppendV1 {
                kind,
                row,
                response: with_response.then_some(PublicationUploadResponseV1 {
                    class: UploadResponseClassV1::Success,
                    detail: "synthetic position".to_string(),
                }),
                observation: with_observation.then_some(PublicationRegistryObservationV1 {
                    provider_reachable: true,
                    row_visible: true,
                    archive_digest_matches: true,
                }),
                at_unix_seconds: at,
                reason: "synthetic".to_string(),
            },
        )
        .map_err(io::Error::other)
    };
    append(
        PublicationJournalEventV1::OperationSelected,
        None,
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::AuthorizationConsumed,
        None,
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::TagObservedExact,
        None,
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::RowPreflightComplete,
        Some(row.clone()),
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::UploadIntentDurable,
        Some(row.clone()),
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::UploadRequestStarted,
        Some(row.clone()),
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::UploadResponseObserved,
        Some(row.clone()),
        true,
        false,
    )?;
    append(
        PublicationJournalEventV1::RegistryObservationStarted,
        Some(row.clone()),
        false,
        false,
    )?;
    append(
        PublicationJournalEventV1::RegistryVisibleExact,
        Some(row.clone()),
        false,
        true,
    )?;
    append(
        PublicationJournalEventV1::OperationComplete,
        None,
        false,
        false,
    )?;
    Ok(journal)
}

/// Full zero-mutation rehearsal through the production state machine: one
/// canonical identity/head composes every child from selection to terminal
/// settlement. Every transition is a pure constructor: no upload, no tag
/// creation, no provider call, no credential read, no live-control change.
#[test]
fn zero_mutation_rehearsal_reaches_terminal_settlement() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Asset, Operation, Package};

    let identity = canonical_identity("rehearsal-0001")?;
    let identity_digest =
        release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?;

    // Canonical event chain, terminal CompleteClean, through SettlementRequired.
    let mut events: Vec<CargoAllowReleaseOperationEventV1> = Vec::new();
    let mut ordinal = 1;
    let mut next = || {
        ordinal += 1;
        ordinal - 1
    };
    rehearsal_append(
        &identity,
        &mut events,
        Event::OperationSelected,
        Operation,
        next(),
    )?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::AuthorizationSelected,
        Operation,
        next(),
    )?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::LeaseAcquired,
        Operation,
        next(),
    )?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::TagIntentDurable,
        Operation,
        next(),
    )?;
    rehearsal_append_request(&identity, &mut events, Operation, next())?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::TagObservedExact,
        Operation,
        next(),
    )?;
    for package in identity.packages.clone() {
        rehearsal_append(
            &identity,
            &mut events,
            Event::PackageRowIntentDurable,
            Package(package.logical_id.clone()),
            next(),
        )?;
        rehearsal_append_request(
            &identity,
            &mut events,
            Package(package.logical_id.clone()),
            next(),
        )?;
        rehearsal_append(
            &identity,
            &mut events,
            Event::PackageRowObservedExact,
            Package(package.logical_id),
            next(),
        )?;
    }
    rehearsal_append_request(&identity, &mut events, Operation, next())?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::GitHubDraftObservedExact,
        Operation,
        next(),
    )?;
    for asset in identity.assets.clone() {
        rehearsal_append_request(
            &identity,
            &mut events,
            Asset(asset.asset_id.clone()),
            next(),
        )?;
        rehearsal_append(
            &identity,
            &mut events,
            Event::AssetObservedExact,
            Asset(asset.asset_id),
            next(),
        )?;
    }
    rehearsal_append_request(&identity, &mut events, Operation, next())?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::PublicReleaseObservedExact,
        Operation,
        next(),
    )?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::RepositoryReconciled,
        Operation,
        next(),
    )?;
    let head_before_settle: CargoAllowReleaseOperationHeadV1 =
        allow_report::compile_release_operation_head_v1(&identity, &events, EVALUATED_AT)
            .map_err(io::Error::other)?;
    require(
        head_before_settle.state == CargoAllowReleaseOperationStateV1::SettlementRequired,
        "rehearsal must pass through settlement-required before settlement",
    )?;
    rehearsal_append(
        &identity,
        &mut events,
        Event::OperationSettled,
        Operation,
        next(),
    )?;
    let evaluation = evaluate_release_operation_v1(&identity, &events, EVALUATED_AT)
        .map_err(io::Error::other)?;
    require(
        evaluation.state == CargoAllowReleaseOperationStateV1::CompleteClean,
        "rehearsal must terminate CompleteClean with zero mutations",
    )?;

    // Journal runs its full row lifecycle to OperationComplete.
    let journal = rehearsal_journal(&identity)?;
    require(
        journal
            .entries
            .iter()
            .any(|entry| entry.kind == PublicationJournalEventV1::OperationComplete),
        "rehearsal journal must complete its row lifecycle",
    )?;
    let journal_head = journal
        .entries
        .last()
        .ok_or_else(|| io::Error::other("completed journal must have a head"))?;

    // Checkpoint gates the completed prefix with independent readback.
    let head = allow_report::compile_release_operation_head_v1(&identity, &events, EVALUATED_AT)
        .map_err(io::Error::other)?;
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
            first_irreversible_row: Some("cargo-allow".to_string()),
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

    // Lease acquires against the live heads and records the irreversible start.
    let mut lease = acquire_operation_lease_for_operation_v1(
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
            checkpoint_head_digest: digest_publication_checkpoint_link_v1(&checkpoint)
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
    note_lease_irreversible_start_v1(&mut lease, NOW + 10).map_err(io::Error::other)?;

    // Tag commits to the identity plus the exact held lease key, then records
    // intent, start, and the observed response without any push.
    let mut tag = begin_tag_transaction_for_operation_v1(
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
    let intent = tag_push_intent_digest_v1(
        &tag.transaction_id,
        &tag.tag.tag_object_id,
        &journal_head.entry_digest,
    )
    .map_err(io::Error::other)?;
    record_tag_push_intent_v1(
        &mut tag,
        FinalTagDurabilityV1 {
            journal_head_digest: journal_head.entry_digest.clone(),
            checkpoint_digest: digest_publication_checkpoint_link_v1(&checkpoint)
                .map_err(io::Error::other)?,
            checkpoint_bound_intent_digest: intent,
        },
        NOW + 20,
    )
    .map_err(io::Error::other)?;
    record_tag_push_started_v1(&mut tag, NOW + 30).map_err(io::Error::other)?;
    record_tag_push_response_v1(&mut tag, true, NOW + 40).map_err(io::Error::other)?;
    require(
        tag.operation_identity_digest == identity_digest
            && matches!(
                tag.state,
                allow_report::TagTransactionStateV1::PushResponseObserved
            ),
        "rehearsal tag must observe its push response under the canonical identity",
    )?;
    Ok(())
}
