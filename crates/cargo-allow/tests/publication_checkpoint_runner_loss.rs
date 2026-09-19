//! Runner-loss and recovery semantics for publication checkpoints (#3922).
//!
//! Synthetic subjects only: no network access, no uploads, no provider API
//! calls, no credentials, and nothing leaves the process. A lost runner is
//! modeled by dropping every local handle and rebuilding state from exact
//! remote evidence only. These tests prove a fresh runner reconstructs the
//! latest honest operation state, stale or foreign evidence never authorizes
//! progress, outage is never absence, and later clean records can never
//! overwrite incident history.

use std::error::Error;
use std::io;

use allow_report::{
    CargoAllowPublicationCheckpointV1, CargoAllowPublicationJournalV1, CheckpointProviderOutcomeV1,
    PublicationCheckpointClassV1, PublicationCheckpointInitV1, PublicationCheckpointKindV1,
    PublicationCheckpointProducerV1, PublicationCheckpointProviderObjectV1,
    PublicationCheckpointProviderV1, PublicationCheckpointReadbackV1,
    PublicationCheckpointRowStateV1, PublicationCheckpointRowV1, PublicationJournalAppendV1,
    PublicationJournalClassV1, PublicationJournalEventV1, PublicationJournalInitV1,
    PublicationJournalRowV1, append_journal_event_v1, begin_publication_checkpoint_v1,
    begin_publication_journal_v1, checkpoint_permits_dependant_v1, checkpoint_permits_upload_v1,
    digest_publication_checkpoint_body_v1, record_checkpoint_readback_v1, render_publication_checkpoint_v1,
    select_checkpoint_by_exact_identity_v1, verify_checkpoint_against_journal_v1,
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

fn journal_row(name: &str, order: u32, n: u64) -> PublicationJournalRowV1 {
    PublicationJournalRowV1 {
        package_name: name.to_string(),
        version: "0.2.0".to_string(),
        row_order: order,
        candidate_archive_digest: digest(n),
        depends_on: Vec::new(),
    }
}

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
    object_id: &str,
) -> Result<PublicationCheckpointInitV1, Box<dyn Error>> {
    let last = match journal.entries.last() {
        Some(entry) => entry,
        None => return fail("checkpoint tests require a settled journal head"),
    };
    Ok(PublicationCheckpointInitV1 {
        checkpoint_id: format!("checkpoint-0-2-0-{sequence:03}"),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_class: PublicationCheckpointClassV1::CleanFinalPublication,
        authorization_digest: digest(70),
        custody_digest: digest(71),
        freeze_digest: digest(72),
        journal_head_sequence: last.sequence,
        journal_head_digest: last.entry_digest.clone(),
        checkpoint_sequence: sequence,
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
            object_id: object_id.to_string(),
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

fn read_back(
    checkpoint: &mut CargoAllowPublicationCheckpointV1,
    bytes: Vec<u8>,
    at: u64,
) -> Result<PublicationCheckpointReadbackV1, Box<dyn Error>> {
    Ok(record_checkpoint_readback_v1(
        checkpoint,
        CheckpointProviderOutcomeV1::Delivered(bytes),
        at,
    )
    .map_err(io::Error::other)?)
}

#[test]
fn publication_checkpoint_runner_loss() -> Result<(), Box<dyn Error>> {
    let journal = settled_journal()?;
    let expected_producer = producer();
    let now = CREATED_AT + 60;
    // A stored pre-intent checkpoint with a verified readback.
    let mut first =
        begin_publication_checkpoint_v1(checkpoint_init(&journal, 1, "artifact-1")?, None)
            .map_err(io::Error::other)?;
    let stored_first = store_checkpoint(&mut first)?;
    read_back(&mut first, stored_first.clone(), now)?;
    // Control: runner lost after remote pre-intent, before upload. The fresh
    // runner holds no local handles: it discovers the checkpoint by exact
    // identity and verifies it against the surviving journal, then uploads.
    // It must not record a second pre-intent for the same row.
    let mut reconstructed: CargoAllowPublicationCheckpointV1 =
        serde_json::from_slice(&stored_first)?;
    require(
        reconstructed.readback == PublicationCheckpointReadbackV1::Missing,
        "remote bytes must not self-claim a successful readback",
    )?;
    read_back(&mut reconstructed, stored_first.clone(), now + 1)?;
    let remote: Vec<CargoAllowPublicationCheckpointV1> = vec![reconstructed];
    let discovered = match select_checkpoint_by_exact_identity_v1(
        &remote,
        "artifact-1",
        &expected_producer,
        "publish_cargo_allow_final_0_2_0",
    )
    .map_err(io::Error::other)?
    {
        Some(checkpoint) => checkpoint.clone(),
        None => return fail("exact checkpoint discovery must succeed"),
    };
    require(
        discovered.checkpoint_sequence == 1
            && discovered.readback == PublicationCheckpointReadbackV1::Complete,
        "discovery must return the verified pre-intent checkpoint",
    )?;
    checkpoint_permits_upload_v1(&discovered, &journal, &expected_producer, now)
        .map_err(io::Error::other)?;
    // Control: runner lost after registry acceptance, before the
    // post-observation checkpoint. Without a verified post-observation
    // checkpoint the dependant row never begins, even though the journal
    // advanced: absence of remote evidence is not evidence of absence.
    require(
        checkpoint_permits_dependant_v1(&discovered, &journal, &expected_producer, now).is_err(),
        "a pre-intent checkpoint must never unlock dependants",
    )?;
    // Control: same checkpoint name from another run is never selected.
    // Discovery takes the exact object ID, never the name.
    let mut foreign_init = checkpoint_init(&journal, 1, "artifact-9")?;
    foreign_init.checkpoint_id = "checkpoint-foreign-001".to_string();
    foreign_init.producer.run = "9999".to_string();
    let mut foreign =
        begin_publication_checkpoint_v1(foreign_init, None).map_err(io::Error::other)?;
    let stored_foreign = store_checkpoint(&mut foreign)?;
    read_back(&mut foreign, stored_foreign, now)?;
    let crowded: Vec<CargoAllowPublicationCheckpointV1> = vec![foreign.clone(), first.clone()];
    let picked = match select_checkpoint_by_exact_identity_v1(
        &crowded,
        "artifact-1",
        &expected_producer,
        "publish_cargo_allow_final_0_2_0",
    )
    .map_err(io::Error::other)?
    {
        Some(checkpoint) => checkpoint,
        None => return fail("exact discovery must skip same-name foreign objects"),
    };
    require(
        picked.checkpoint_id == "checkpoint-0-2-0-001",
        "discovery must return the exact-ID object, not the same-name one",
    )?;
    require(
        select_checkpoint_by_exact_identity_v1(
            &crowded,
            "artifact-9",
            &expected_producer,
            "publish_cargo_allow_final_0_2_0",
        )
        .map_err(io::Error::other)?
        .is_none(),
        "a foreign-producer object ID must never resolve under our producer",
    )?;
    require(
        select_checkpoint_by_exact_identity_v1(
            &crowded,
            "publication-checkpoint",
            &expected_producer,
            "publish_cargo_allow_final_0_2_0",
        )
        .map_err(io::Error::other)?
        .is_none(),
        "bare object names must never resolve to a checkpoint",
    )?;
    let duplicate_exact = vec![discovered.clone(), discovered.clone()];
    require(
        select_checkpoint_by_exact_identity_v1(
            &duplicate_exact,
            "artifact-1",
            &expected_producer,
            "publish_cargo_allow_final_0_2_0",
        )
        .is_err(),
        "duplicate exact provider identities must fail discovery as ambiguous",
    )?;

    // Control: checkpoint prefix does not match the live journal. A
    // truncated journal (head entry lost) breaks prefix verification.
    let mut truncated = journal.clone();
    truncated.entries.pop();
    require(
        verify_checkpoint_against_journal_v1(&discovered, &truncated, &expected_producer, now)
            .is_err(),
        "a truncated journal prefix must fail verification",
    )?;
    let mut rebound = checkpoint_init(&journal, 1, "artifact-1")?;
    rebound.journal_head_digest = digest(999);
    let rebound_record =
        begin_publication_checkpoint_v1(rebound, None).map_err(io::Error::other)?;
    require(
        verify_checkpoint_against_journal_v1(&rebound_record, &journal, &expected_producer, now)
            .is_err(),
        "a rebound head digest must fail verification",
    )?;
    // Control: an expired checkpoint never authorizes progress, even with a
    // Complete readback and an intact prefix.
    let expired_at = first.expires_at_unix_seconds + 1;
    require(
        verify_checkpoint_against_journal_v1(&discovered, &journal, &expected_producer, expired_at)
            .is_err(),
        "expired checkpoints must fail verification",
    )?;
    // Control: provider outage is not absence. Unavailable readbacks block
    // progress and are never coerced into row-absent claims: the readback
    // enum has no absent variant to coerce into.
    let mut outage =
        begin_publication_checkpoint_v1(checkpoint_init(&journal, 1, "artifact-1")?, None)
            .map_err(io::Error::other)?;
    let stored_outage = store_checkpoint(&mut outage)?;
    read_back(&mut outage, stored_outage, now)?;
    record_checkpoint_readback_v1(
        &mut outage,
        CheckpointProviderOutcomeV1::Unavailable,
        now + 10,
    )
    .map_err(io::Error::other)?;
    require(
        outage.readback == PublicationCheckpointReadbackV1::ProviderUnavailable,
        "outages must persist as unavailable",
    )?;
    require(
        checkpoint_permits_upload_v1(&outage, &journal, &expected_producer, now + 10).is_err(),
        "outage readbacks must block uploads",
    )?;
    // Control: later clean state must not overwrite incident history. A
    // checkpoint recording the incident links cleanly; any later checkpoint
    // clearing the incident or renaming the first irreversible row fails.
    let mut incident_init = checkpoint_init(&journal, 2, "artifact-2")?;
    incident_init.kind = PublicationCheckpointKindV1::PostObservation;
    incident_init.row.state = PublicationCheckpointRowStateV1::Incident;
    incident_init.first_irreversible_row = Some("cargo-allow".to_string());
    incident_init.incident_recorded = true;
    let mut incident =
        begin_publication_checkpoint_v1(incident_init, Some(&discovered)).map_err(io::Error::other)?;
    require(
        incident.prior_checkpoint_digest.is_some(),
        "the incident checkpoint must link its reconstructed predecessor",
    )?;
    let stored_incident = store_checkpoint(&mut incident)?;
    read_back(&mut incident, stored_incident, CREATED_AT + 160)?;
    let mut cleared_init = checkpoint_init(&journal, 3, "artifact-3")?;
    cleared_init.kind = PublicationCheckpointKindV1::PostObservation;
    require(
        begin_publication_checkpoint_v1(cleared_init, Some(&incident)).is_err(),
        "clearing incident posture must fail",
    )?;
    let mut renamed_init = checkpoint_init(&journal, 3, "artifact-3")?;
    renamed_init.kind = PublicationCheckpointKindV1::PostObservation;
    renamed_init.first_irreversible_row = Some("allow-report".to_string());
    renamed_init.incident_recorded = true;
    require(
        begin_publication_checkpoint_v1(renamed_init, Some(&incident)).is_err(),
        "renaming the first irreversible row must fail",
    )?;
    let mut continued_init = checkpoint_init(&journal, 3, "artifact-3")?;
    continued_init.kind = PublicationCheckpointKindV1::PostObservation;
    continued_init.row.state = PublicationCheckpointRowStateV1::Incident;
    continued_init.first_irreversible_row = Some("cargo-allow".to_string());
    continued_init.incident_recorded = true;
    let continued = begin_publication_checkpoint_v1(continued_init, Some(&incident))
        .map_err(io::Error::other)?;
    require(
        continued.checkpoint_sequence == 3 && continued.incident_recorded,
        "incident posture must carry forward exactly",
    )?;
    Ok(())
}
