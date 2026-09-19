//! Remotely durable publication checkpoints for total runner loss (#3922).
//!
//! Synthetic subjects only: no network access, no uploads, no provider API
//! calls, no credentials, and nothing leaves the process. Provider outcomes
//! are caller-supplied bytes and outage reports; the checkpoint classifies
//! them but never fetches them. These tests prove exact operation and
//! journal-prefix identity, monotonic linkage, immutable provider object
//! identity, producer trust, retention, schema parity, and the readback law:
//! provider success without readback is never clean.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use allow_report::{
    CargoAllowPublicationCheckpointV1, CargoAllowPublicationJournalV1, CheckpointProviderOutcomeV1,
    PUBLICATION_CHECKPOINT_SCHEMA_ID, PUBLICATION_CHECKPOINT_SCHEMA_VERSION,
    PublicationCheckpointClassV1, PublicationCheckpointInitV1, PublicationCheckpointKindV1,
    PublicationCheckpointProducerV1, PublicationCheckpointProviderObjectV1,
    PublicationCheckpointProviderV1, PublicationCheckpointReadbackV1,
    PublicationCheckpointRowStateV1, PublicationCheckpointRowV1, PublicationJournalAppendV1,
    PublicationJournalClassV1, PublicationJournalEventV1, PublicationJournalInitV1,
    PublicationJournalRowV1, PublicationRegistryObservationV1, PublicationUploadResponseV1,
    UploadResponseClassV1, append_journal_event_v1, begin_publication_checkpoint_v1,
    begin_publication_journal_v1, checkpoint_permits_dependant_v1, checkpoint_permits_upload_v1,
    digest_publication_checkpoint_body_v1, record_checkpoint_readback_v1,
    render_publication_checkpoint_v1, verify_checkpoint_against_journal_v1,
};

const CREATED_AT: u64 = 1_786_200_000;
const RETENTION_DAYS: u32 = 30;

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

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    match manifest_dir.parent() {
        Some(crates_dir) => match crates_dir.parent() {
            Some(root) => Ok(root.to_path_buf()),
            None => fail("cargo-allow crates directory has no repository parent"),
        },
        None => fail("cargo-allow manifest has no crates parent"),
    }
}

fn journal_row(name: &str, order: u32, n: u64) -> PublicationJournalRowV1 {
    PublicationJournalRowV1 {
        package_name: name.to_string(),
        version: "0.2.0".to_string(),
        row_order: order,
        candidate_archive_digest: digest(n),
        depends_on: Vec::new(),
    }
}

/// A journal settled through durable intent: five entries, head at 5.
fn settled_journal() -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    let mut journal = begin_publication_journal_v1(PublicationJournalInitV1 {
        journal_id: "journal-0-2-0-001".to_string(),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_class: PublicationJournalClassV1::CleanFinalPublication,
        authorization_digest: digest(70),
        custody_digest: digest(71),
        freeze_digest: digest(72),
        prior_journal_digest: None,
        rows: vec![journal_row("cargo-allow", 0, 10)],
        created_at_unix_seconds: CREATED_AT + sequence.saturating_sub(1) * 100,
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "publish".to_string(),
    })
    .map_err(io::Error::other)?;
    let row = journal_row("cargo-allow", 0, 10);
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let mut append = |journal: &mut CargoAllowPublicationJournalV1,
                      kind: PublicationJournalEventV1,
                      row: Option<PublicationJournalRowV1>| {
        append_journal_event_v1(
            journal,
            PublicationJournalAppendV1 {
                kind,
                row,
                response: None,
                observation: None,
                at_unix_seconds: next(),
                reason: "synthetic".to_string(),
            },
        )
        .map_err(io::Error::other)
    };
    append(
        &mut journal,
        PublicationJournalEventV1::OperationSelected,
        None,
    )?;
    append(
        &mut journal,
        PublicationJournalEventV1::AuthorizationConsumed,
        None,
    )?;
    append(
        &mut journal,
        PublicationJournalEventV1::TagObservedExact,
        None,
    )?;
    append(
        &mut journal,
        PublicationJournalEventV1::RowPreflightComplete,
        Some(row.clone()),
    )?;
    append(
        &mut journal,
        PublicationJournalEventV1::UploadIntentDurable,
        Some(row),
    )?;
    Ok(journal)
}

fn advance_to_visible_exact(
    journal: &mut CargoAllowPublicationJournalV1,
) -> Result<(), Box<dyn Error>> {
    let row = journal
        .rows
        .first()
        .cloned()
        .ok_or_else(|| io::Error::other("checkpoint fixture row absent"))?;
    let mut at = journal
        .entries
        .last()
        .map(|entry| entry.at_unix_seconds)
        .unwrap_or(CREATED_AT);
    let mut next = || {
        at += 10;
        at
    };
    append_journal_event_v1(
        journal,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::UploadRequestStarted,
            row: Some(row.clone()),
            response: None,
            observation: None,
            at_unix_seconds: next(),
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        journal,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::UploadResponseObserved,
            row: Some(row.clone()),
            response: Some(PublicationUploadResponseV1 {
                class: UploadResponseClassV1::Success,
                detail: "synthetic".to_string(),
            }),
            observation: None,
            at_unix_seconds: next(),
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        journal,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::RegistryObservationStarted,
            row: Some(row.clone()),
            response: None,
            observation: None,
            at_unix_seconds: next(),
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        journal,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::RegistryVisibleExact,
            row: Some(row),
            response: None,
            observation: Some(PublicationRegistryObservationV1 {
                provider_reachable: true,
                row_visible: true,
                archive_digest_matches: true,
            }),
            at_unix_seconds: next(),
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    Ok(())
}

fn producer() -> PublicationCheckpointProducerV1 {
    PublicationCheckpointProducerV1 {
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "publish".to_string(),
        git_ref: "refs/tags/v0.2.0".to_string(),
        commit: "4d1b0fe10e82f506424f9dc7b7ff3bf81403191b".to_string(),
    }
}

fn checkpoint_init(
    journal: &CargoAllowPublicationJournalV1,
    sequence: u64,
    kind: PublicationCheckpointKindV1,
) -> Result<PublicationCheckpointInitV1, Box<dyn Error>> {
    let last = match journal.entries.last() {
        Some(entry) => entry,
        None => return fail("checkpoint tests require a settled journal head"),
    };
    let head_sequence = last.sequence;
    let head_digest = last.entry_digest.clone();
    Ok(PublicationCheckpointInitV1 {
        checkpoint_id: format!("checkpoint-0-2-0-{sequence:03}"),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_class: PublicationCheckpointClassV1::CleanFinalPublication,
        authorization_digest: digest(70),
        custody_digest: digest(71),
        freeze_digest: digest(72),
        journal_head_sequence: head_sequence,
        journal_head_digest: head_digest,
        checkpoint_sequence: sequence,
        kind,
        row: PublicationCheckpointRowV1 {
            package_name: "cargo-allow".to_string(),
            row_order: 0,
            state: PublicationCheckpointRowStateV1::IntentDurable,
        },
        first_irreversible_row: None,
        incident_recorded: false,
        provider: PublicationCheckpointProviderObjectV1 {
            provider: PublicationCheckpointProviderV1::GithubActionsArtifact,
            object_id: format!("artifact-{sequence}"),
            object_name: "publication-checkpoint".to_string(),
            object_digest: digest(0),
            object_size_bytes: 1,
        },
        producer: producer(),
        retention_days: RETENTION_DAYS,
        created_at_unix_seconds: CREATED_AT,
        note: "synthetic".to_string(),
    })
}

/// Mirror the real producer protocol: render, bind the body digest, measure,
/// and converge the recorded size to the stored bytes by fixpoint.
fn store_checkpoint(
    checkpoint: &mut CargoAllowPublicationCheckpointV1,
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

fn set_field(
    value: &mut serde_json::Value,
    key: &str,
    field: serde_json::Value,
) -> Result<(), Box<dyn Error>> {
    match value.as_object_mut() {
        Some(object) => {
            object.insert(key.to_string(), field);
            Ok(())
        }
        None => fail("checkpoint JSON not an object"),
    }
}

#[test]
fn publication_checkpoint() -> Result<(), Box<dyn Error>> {
    let mut journal = settled_journal()?;
    let expected_producer = producer();
    let now = CREATED_AT + 60;
    // A fresh checkpoint reads back Missing: provider success without
    // readback is never clean.
    let mut first = begin_publication_checkpoint_v1(
        checkpoint_init(&journal, 1, PublicationCheckpointKindV1::PreIntentDurable)?,
        None,
    )
    .map_err(io::Error::other)?;
    require(
        first.readback == PublicationCheckpointReadbackV1::Missing,
        "fresh checkpoints must read back Missing",
    )?;
    require(
        first.prior_checkpoint_digest.is_none(),
        "sequence one carries no prior digest",
    )?;
    require(
        first.expires_at_unix_seconds == CREATED_AT + u64::from(RETENTION_DAYS) * 86_400,
        "expiry must equal construction plus retention exactly",
    )?;
    require(
        checkpoint_permits_upload_v1(&first, &journal, &expected_producer, now).is_err(),
        "uploads must not begin before a verified readback",
    )?;
    // Store the exact bytes, then read them back independently.
    let stored = store_checkpoint(&mut first)?;
    let readback = record_checkpoint_readback_v1(
        &mut first,
        CheckpointProviderOutcomeV1::Delivered(stored.clone()),
        now,
    )
    .map_err(io::Error::other)?;
    require(
        readback == PublicationCheckpointReadbackV1::Complete,
        "exact stored bytes must read back Complete",
    )?;
    verify_checkpoint_against_journal_v1(&first, &journal, &expected_producer, now)
        .map_err(io::Error::other)?;
    checkpoint_permits_upload_v1(&first, &journal, &expected_producer, now)
        .map_err(io::Error::other)?;
    require(
        checkpoint_permits_dependant_v1(&first, &journal, &expected_producer, now).is_err(),
        "a pre-intent checkpoint must not unlock dependants",
    )?;
    // Advance the real journal through upload and exact registry visibility.
    // The post-observation checkpoint must bind that exact clean transition.
    advance_to_visible_exact(&mut journal)?;
    let mut second_init =
        checkpoint_init(&journal, 2, PublicationCheckpointKindV1::PostObservation)?;
    second_init.row.state = PublicationCheckpointRowStateV1::VisibleExact;
    second_init.first_irreversible_row = Some("cargo-allow".to_string());
    let mut second =
        begin_publication_checkpoint_v1(second_init, Some(&first)).map_err(io::Error::other)?;
    require(
        second.prior_checkpoint_digest.is_some(),
        "sequence two must bind its predecessor",
    )?;
    let stored_second = store_checkpoint(&mut second)?;
    record_checkpoint_readback_v1(
        &mut second,
        CheckpointProviderOutcomeV1::Delivered(stored_second),
        CREATED_AT + 160,
    )
    .map_err(io::Error::other)?;
    checkpoint_permits_dependant_v1(&second, &journal, &expected_producer, CREATED_AT + 160)
        .map_err(io::Error::other)?;
    require(
        checkpoint_permits_upload_v1(&second, &journal, &expected_producer, CREATED_AT + 160)
            .is_err(),
        "a post-observation checkpoint must not authorize uploads",
    )?;
    // Hostile: non-clean post-observation states never unlock dependants.
    for state in [
        PublicationCheckpointRowStateV1::VisibleConflict,
        PublicationCheckpointRowStateV1::ObservedAbsent,
        PublicationCheckpointRowStateV1::Waiting,
        PublicationCheckpointRowStateV1::ResponseUnknown,
        PublicationCheckpointRowStateV1::Incident,
    ] {
        let mut hostile = second.clone();
        hostile.row.state = state;
        require(
            checkpoint_permits_dependant_v1(
                &hostile,
                &journal,
                &expected_producer,
                CREATED_AT + 160,
            )
            .is_err(),
            format!("post-observation state {state:?} must not unlock dependants"),
        )?;
    }
    // Hostile: a wrong event/prefix cannot authorize even with a clean row claim.
    let mut wrong_prefix = second.clone();
    wrong_prefix.journal_head_sequence = first.journal_head_sequence;
    wrong_prefix.journal_head_digest = first.journal_head_digest.clone();
    wrong_prefix.first_irreversible_row = None;
    require(
        checkpoint_permits_dependant_v1(
            &wrong_prefix,
            &journal,
            &expected_producer,
            CREATED_AT + 160,
        )
        .is_err(),
        "visible-exact claims bound to a durable-intent prefix must fail",
    )?;

    // Control: the rendered checkpoints validate against the schema, with
    // the same sequence-one-null / later-digest prior law as the journal.
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.publication-checkpoint.v1.schema.json"),
        )?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("checkpoint schema compiles: {error}")))?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_publication_checkpoint_v1(&first)?)?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!("stored checkpoint must validate: {error}"))
        })?;
        require(
            rendered.get("schema_id")
                == Some(&serde_json::Value::String(
                    PUBLICATION_CHECKPOINT_SCHEMA_ID.into(),
                ))
                && rendered.get("schema_version")
                    == Some(&serde_json::Value::from(
                        PUBLICATION_CHECKPOINT_SCHEMA_VERSION,
                    )),
            "rendered checkpoint must carry its schema identity",
        )?;
        let mut bound_first = rendered.clone();
        set_field(
            &mut bound_first,
            "prior_checkpoint_digest",
            serde_json::Value::String(digest(99)),
        )?;
        require(
            validator.validate(&bound_first).is_err(),
            "sequence one must not bind a prior digest in schema",
        )?;
        let rendered_second: serde_json::Value =
            serde_json::from_str(&render_publication_checkpoint_v1(&second)?)?;
        validator.validate(&rendered_second).map_err(|error| {
            io::Error::other(format!("linked checkpoint must validate: {error}"))
        })?;
        let mut null_second = rendered_second.clone();
        set_field(
            &mut null_second,
            "prior_checkpoint_digest",
            serde_json::Value::Null,
        )?;
        require(
            validator.validate(&null_second).is_err(),
            "later sequences require a prior digest in schema",
        )?;
    }
    // Control: linkage faults fail closed at construction.
    require(
        begin_publication_checkpoint_v1(
            checkpoint_init(&journal, 2, PublicationCheckpointKindV1::PreIntentDurable)?,
            None,
        )
        .is_err(),
        "sequence two without a predecessor must fail",
    )?;
    require(
        begin_publication_checkpoint_v1(
            checkpoint_init(&journal, 1, PublicationCheckpointKindV1::PreIntentDurable)?,
            Some(&first),
        )
        .is_err(),
        "sequence one with a predecessor must fail",
    )?;
    let mut skipped = checkpoint_init(&journal, 3, PublicationCheckpointKindV1::PreIntentDurable)?;
    skipped.checkpoint_id = "checkpoint-0-2-0-003".to_string();
    require(
        begin_publication_checkpoint_v1(skipped, Some(&first)).is_err(),
        "sequence gaps must fail",
    )?;
    Ok(())
}
