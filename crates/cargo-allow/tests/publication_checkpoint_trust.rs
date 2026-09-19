//! Producer trust, readback classification, and retention for checkpoints (#3922).
//!
//! Synthetic subjects only: no network access, no uploads, no provider API
//! calls, no credentials, and nothing leaves the process. Trust is exact
//! typed producer equality against the identity the release lane expects;
//! readback classification is fail-closed over caller-supplied bytes. These
//! tests prove untrusted producers never authorize progress, every byte
//! fault classifies honestly, retention bounds hold, and secrets never enter
//! checkpoint records.

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
    begin_publication_journal_v1, checkpoint_permits_upload_v1,
    digest_publication_checkpoint_body_v1, record_checkpoint_readback_v1,
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
            object_id: "artifact-1".to_string(),
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
        let bytes = serde_json::to_vec_pretty(checkpoint)?;
        let len = bytes.len() as u64;
        if checkpoint.provider.object_size_bytes == len {
            return Ok(bytes);
        }
        checkpoint.provider.object_size_bytes = len;
    }
    fail("checkpoint stored size must converge")
}

fn stored_first(
    journal: &CargoAllowPublicationJournalV1,
) -> Result<(CargoAllowPublicationCheckpointV1, Vec<u8>), Box<dyn Error>> {
    let mut first = begin_publication_checkpoint_v1(checkpoint_init(journal, 1)?, None)
        .map_err(io::Error::other)?;
    let bytes = store_checkpoint(&mut first)?;
    Ok((first, bytes))
}

#[test]
fn publication_checkpoint_trust() -> Result<(), Box<dyn Error>> {
    let journal = settled_journal()?;
    let expected_producer = producer();
    let now = CREATED_AT + 60;
    // Hostile: an untrusted producer (fork job reusing the object ID and
    // operation) is never selected and never verifies.
    let (mut trusted, stored) = stored_first(&journal)?;
    record_checkpoint_readback_v1(
        &mut trusted,
        CheckpointProviderOutcomeV1::Delivered(stored),
        now,
    )
    .map_err(io::Error::other)?;
    let mut fork_producer = producer();
    fork_producer.run = "7777".to_string();
    require(
        select_checkpoint_by_exact_identity_v1(
            &[trusted.clone()],
            "artifact-1",
            &fork_producer,
            "publish_cargo_allow_final_0_2_0",
        )
        .map_err(io::Error::other)?
        .is_none(),
        "fork producers must never resolve a checkpoint",
    )?;
    require(
        verify_checkpoint_against_journal_v1(&trusted, &journal, &fork_producer, now).is_err(),
        "fork producers must never verify a checkpoint",
    )?;
    // Hostile: every byte fault classifies honestly, never Complete.
    let (mut tampered, _) = stored_first(&journal)?;
    let mut bad_bytes = store_checkpoint(&mut tampered)?;
    match bad_bytes.last_mut() {
        Some(byte) => *byte ^= 0x01,
        None => return fail("stored checkpoint bytes absent"),
    }
    let verdict = record_checkpoint_readback_v1(
        &mut tampered,
        CheckpointProviderOutcomeV1::Delivered(bad_bytes),
        now,
    )
    .map_err(io::Error::other)?;
    require(
        verdict == PublicationCheckpointReadbackV1::Mismatch,
        "flipped stored bytes must read back Mismatch",
    )?;
    let (mut truncated, _) = stored_first(&journal)?;
    let mut short_bytes = store_checkpoint(&mut truncated)?;
    short_bytes.pop();
    let verdict = record_checkpoint_readback_v1(
        &mut truncated,
        CheckpointProviderOutcomeV1::Delivered(short_bytes),
        now,
    )
    .map_err(io::Error::other)?;
    require(
        verdict == PublicationCheckpointReadbackV1::Mismatch,
        "truncated stored bytes must read back Mismatch",
    )?;
    let (mut garbage, _) = stored_first(&journal)?;
    let verdict = record_checkpoint_readback_v1(
        &mut garbage,
        CheckpointProviderOutcomeV1::Delivered(b"not checkpoint json".to_vec()),
        now,
    )
    .map_err(io::Error::other)?;
    require(
        verdict == PublicationCheckpointReadbackV1::Mismatch,
        "unparsable stored bytes must read back Mismatch",
    )?;
    let (mut empty, _) = stored_first(&journal)?;
    let verdict = record_checkpoint_readback_v1(
        &mut empty,
        CheckpointProviderOutcomeV1::Delivered(Vec::new()),
        now,
    )
    .map_err(io::Error::other)?;
    require(
        verdict == PublicationCheckpointReadbackV1::Mismatch,
        "empty stored bytes must read back Mismatch",
    )?;
    // Hostile: provider identity is immutable and must match delivered bytes.
    let (mut provider_tamper, provider_bytes) = stored_first(&journal)?;
    let mut provider_json: serde_json::Value = serde_json::from_slice(&provider_bytes)?;
    provider_json["provider"]["object_id"] = serde_json::Value::String("artifact-9".to_string());
    let tampered_provider_bytes = serde_json::to_vec_pretty(&provider_json)?;
    require(
        record_checkpoint_readback_v1(
            &mut provider_tamper,
            CheckpointProviderOutcomeV1::Delivered(tampered_provider_bytes),
            now,
        )
        .map_err(io::Error::other)?
            == PublicationCheckpointReadbackV1::Mismatch,
        "provider-object tampering must read back Mismatch",
    )?;
    // Hostile: observation time never moves backward.
    let (mut monotonic, monotonic_bytes) = stored_first(&journal)?;
    record_checkpoint_readback_v1(
        &mut monotonic,
        CheckpointProviderOutcomeV1::Delivered(monotonic_bytes),
        now + 20,
    )
    .map_err(io::Error::other)?;
    let before = monotonic.clone();
    require(
        record_checkpoint_readback_v1(
            &mut monotonic,
            CheckpointProviderOutcomeV1::Unavailable,
            now + 10,
        )
        .is_err()
            && monotonic == before,
        "older readback observations must fail without mutating the retained state",
    )?;

    // Hostile: older same-operation bytes are Stale, never silently current.
    let (mut current, current_bytes) = stored_first(&journal)?;
    record_checkpoint_readback_v1(
        &mut current,
        CheckpointProviderOutcomeV1::Delivered(current_bytes),
        now,
    )
    .map_err(io::Error::other)?;
    let mut older = current.clone();
    older.checkpoint_sequence = 0;
    let older_rendered = serde_json::to_vec_pretty(&older)?;
    let verdict = record_checkpoint_readback_v1(
        &mut current,
        CheckpointProviderOutcomeV1::Delivered(older_rendered),
        now + 10,
    )
    .map_err(io::Error::other)?;
    require(
        verdict == PublicationCheckpointReadbackV1::Stale,
        "older same-operation bytes must read back Stale",
    )?;
    require(
        checkpoint_permits_upload_v1(&current, &journal, &expected_producer, now + 10).is_err(),
        "stale readbacks must block uploads",
    )?;
    // Hostile: instrument failure blocks progress like any non-clean outcome.
    let (mut broken, bytes) = stored_first(&journal)?;
    record_checkpoint_readback_v1(
        &mut broken,
        CheckpointProviderOutcomeV1::Delivered(bytes),
        now,
    )
    .map_err(io::Error::other)?;
    record_checkpoint_readback_v1(
        &mut broken,
        CheckpointProviderOutcomeV1::InstrumentFailure,
        now + 10,
    )
    .map_err(io::Error::other)?;
    require(
        broken.readback == PublicationCheckpointReadbackV1::InstrumentFailure,
        "instrument failures must persist as instrument failure",
    )?;
    require(
        checkpoint_permits_upload_v1(&broken, &journal, &expected_producer, now + 10).is_err(),
        "instrument failures must block uploads",
    )?;
    // Hostile: retention outside the provider window fails at construction;
    // the window edges pass.
    for days in [0, 91] {
        let mut init = checkpoint_init(&journal, 1)?;
        init.retention_days = days;
        require(
            begin_publication_checkpoint_v1(init, None).is_err(),
            format!("retention {days} days must fail"),
        )?;
    }
    for days in [1, 90] {
        let mut init = checkpoint_init(&journal, 1)?;
        init.retention_days = days;
        init.checkpoint_id = format!("checkpoint-retention-{days:03}");
        let checkpoint = begin_publication_checkpoint_v1(init, None).map_err(io::Error::other)?;
        require(
            checkpoint.expires_at_unix_seconds == CREATED_AT + u64::from(days) * 86_400,
            format!("retention {days} days must set exact expiry"),
        )?;
    }
    let mut overflow = checkpoint_init(&journal, 1)?;
    overflow.created_at_unix_seconds = u64::MAX - 10;
    require(
        begin_publication_checkpoint_v1(overflow, None).is_err(),
        "checkpoint expiry arithmetic must fail closed on overflow",
    )?;

    // Hostile: operation identity binds to its class in both directions.
    let mut wrong_clean = checkpoint_init(&journal, 1)?;
    wrong_clean.operation_id = "recover_cargo_allow_final_publication".to_string();
    require(
        begin_publication_checkpoint_v1(wrong_clean, None).is_err(),
        "clean checkpoints must refuse the recovery operation id",
    )?;
    // Hostile: secret material never enters checkpoint records.
    let mut leaked = checkpoint_init(&journal, 1)?;
    leaked.note = "retry token=synthetic-secret".to_string();
    require(
        begin_publication_checkpoint_v1(leaked, None).is_err(),
        "secret material in a checkpoint note must fail",
    )?;
    // Hostile: checkpoints must not under-report journal incidents. The
    // journal gains an incident after the checkpoint; verification then fails
    // until an incident-bound checkpoint replaces it.
    let mut advanced = journal.clone();
    append_journal_event_v1(
        &mut advanced,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::OperationIncident,
            row: None,
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 100,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    require(
        verify_checkpoint_against_journal_v1(&trusted, &advanced, &expected_producer, now).is_err(),
        "checkpoints must not under-report journal incidents",
    )?;
    // Control: honest journal advancement past the checkpoint keeps
    // verification green: the row-state claim is history, the prefix is law.
    let mut moved = journal.clone();
    append_journal_event_v1(
        &mut moved,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::UploadRequestStarted,
            row: Some(journal_row("cargo-allow", 0, 10)),
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 100,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    verify_checkpoint_against_journal_v1(&trusted, &moved, &expected_producer, now)
        .map_err(io::Error::other)?;
    Ok(())
}
