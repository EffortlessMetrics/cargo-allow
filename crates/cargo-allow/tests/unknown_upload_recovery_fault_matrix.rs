//! Crash-window fault matrix for unknown-upload recovery (#3924).
//!
//! Synthetic subjects only: no network access, no uploads, no registry
//! observation, no credentials, and nothing leaves the process. Each window
//! names the exact crash position in the publication loop and proves the
//! production state machine chooses the one safe disposition: never a blind
//! retry, never an inferred outcome, never a rewritten history.

use std::error::Error;
use std::io;

use PublicationJournalEventV1 as Event;
use UnknownUploadRecoveryClassV1 as Class;
use allow_report::{
    CargoAllowPublicationJournalV1, PublicationCheckpointKindV1, PublicationJournalAppendV1,
    PublicationJournalClassV1, PublicationJournalEventV1, PublicationJournalInitV1,
    PublicationJournalRowV1, PublicationRegistryObservationV1, PublicationUploadResponseV1,
    RecoveryAuthorizationV1, RecoveryAuthorizationsV1, RecoveryCheckpointPositionV1,
    RecoveryCustodyV1, RecoveryRegistryObservationsV1, RecoverySurfaceObservationV1,
    UnknownUploadRecoveryClassV1, UploadResponseClassV1, append_journal_event_v1,
    begin_publication_journal_v1, decide_unknown_upload_recovery_v1,
    recovery_dependant_may_begin_v1,
};

const CREATED_AT: u64 = 1_786_200_000;
const MAX_ROUNDS: u32 = 5;

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

fn journal_row() -> PublicationJournalRowV1 {
    PublicationJournalRowV1 {
        package_name: "cargo-allow".to_string(),
        version: "0.2.0".to_string(),
        row_order: 0,
        candidate_archive_digest: digest(10),
        depends_on: Vec::new(),
    }
}

fn begin_journal(
    rows: Vec<PublicationJournalRowV1>,
) -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    Ok(begin_publication_journal_v1(PublicationJournalInitV1 {
        journal_id: "journal-0-2-0-001".to_string(),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_class: PublicationJournalClassV1::CleanFinalPublication,
        authorization_digest: digest(70),
        custody_digest: digest(71),
        freeze_digest: digest(72),
        prior_journal_digest: None,
        rows,
        created_at_unix_seconds: CREATED_AT,
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "publish".to_string(),
    })
    .map_err(io::Error::other)?)
}

fn ev(
    journal: &mut CargoAllowPublicationJournalV1,
    kind: PublicationJournalEventV1,
    row: Option<PublicationJournalRowV1>,
    response: Option<PublicationUploadResponseV1>,
    observation: Option<PublicationRegistryObservationV1>,
    at: u64,
) -> Result<(), Box<dyn Error>> {
    append_journal_event_v1(
        journal,
        PublicationJournalAppendV1 {
            kind,
            row,
            response,
            observation,
            at_unix_seconds: at,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    Ok(())
}

fn response(class: UploadResponseClassV1) -> PublicationUploadResponseV1 {
    PublicationUploadResponseV1 {
        class,
        detail: "synthetic position".to_string(),
    }
}

fn exact_observation() -> PublicationRegistryObservationV1 {
    PublicationRegistryObservationV1 {
        provider_reachable: true,
        row_visible: true,
        archive_digest_matches: true,
    }
}

/// Journal settled through durable intent: five entries, head at 5.
fn settled() -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    let mut journal = begin_journal(vec![journal_row()])?;
    let row = journal_row();
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    ev(
        &mut journal,
        Event::OperationSelected,
        None,
        None,
        None,
        next(),
    )?;
    ev(
        &mut journal,
        Event::AuthorizationConsumed,
        None,
        None,
        None,
        next(),
    )?;
    ev(
        &mut journal,
        Event::TagObservedExact,
        None,
        None,
        None,
        next(),
    )?;
    ev(
        &mut journal,
        Event::RowPreflightComplete,
        Some(row.clone()),
        None,
        None,
        next(),
    )?;
    ev(
        &mut journal,
        Event::UploadIntentDurable,
        Some(row),
        None,
        None,
        next(),
    )?;
    Ok(journal)
}

fn surface(reachable: bool, visible: bool, matches: Option<bool>) -> RecoverySurfaceObservationV1 {
    RecoverySurfaceObservationV1 {
        reachable,
        visible,
        digest_matches: matches,
    }
}

fn observations(
    api: RecoverySurfaceObservationV1,
    index: RecoverySurfaceObservationV1,
    download: RecoverySurfaceObservationV1,
    resolver: RecoverySurfaceObservationV1,
    rounds: u32,
) -> RecoveryRegistryObservationsV1 {
    RecoveryRegistryObservationsV1 {
        api,
        index,
        download,
        resolver,
        rounds,
        instrument_failure: false,
    }
}

fn absent(rounds: u32) -> RecoveryRegistryObservationsV1 {
    let silent = surface(true, false, None);
    observations(silent, silent, silent, silent, rounds)
}

fn custody() -> RecoveryCustodyV1 {
    RecoveryCustodyV1 {
        candidate_archive_digest: digest(10),
        custody_digest: digest(71),
        freeze_digest: digest(72),
    }
}

fn authorizations() -> RecoveryAuthorizationsV1 {
    RecoveryAuthorizationsV1 {
        clean_authorization_digest: digest(70),
        recovery: None,
    }
}

fn recovery_authorizations() -> RecoveryAuthorizationsV1 {
    RecoveryAuthorizationsV1 {
        clean_authorization_digest: digest(70),
        recovery: Some(RecoveryAuthorizationV1 {
            authorization_digest: digest(80),
            plan_digest: digest(81),
            bound_candidate_digest: digest(10),
        }),
    }
}

fn pre_intent_position(
    journal: &CargoAllowPublicationJournalV1,
) -> Result<RecoveryCheckpointPositionV1, Box<dyn Error>> {
    let head = match journal.entries.last() {
        Some(entry) => entry,
        None => return fail("recovery fixtures require a journal head"),
    };
    Ok(RecoveryCheckpointPositionV1 {
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        checkpoint_sequence: 1,
        journal_head_sequence: head.sequence,
        journal_head_digest: head.entry_digest.clone(),
        kind: PublicationCheckpointKindV1::PreIntentDurable,
        readback_complete: true,
    })
}

#[test]
fn unknown_upload_recovery_fault_matrix() -> Result<(), Box<dyn Error>> {
    let row = journal_row();
    // Windows 1-2: crash before remote pre-intent, and after pre-intent but
    // before process start. Nothing was ever sent: safe to start fresh.
    let clean = settled()?;
    for (window, journal) in [(1, settled()?), (2, settled()?)] {
        let disposition = decide_unknown_upload_recovery_v1(
            &journal,
            &[],
            &row,
            &custody(),
            &authorizations(),
            &absent(0),
        )
        .map_err(io::Error::other)?;
        require(
            disposition.class == Class::NotAttemptedSafeToStart,
            format!("window {window}: never-attempted rows must start fresh"),
        )?;
    }
    let _ = clean;
    // Window 3: crash after process start but before the request reaches the
    // provider. The journal holds a started request with no response.
    let mut started = settled()?;
    ev(
        &mut started,
        Event::UploadRequestStarted,
        Some(row.clone()),
        None,
        None,
        CREATED_AT + 60,
    )?;
    let disposition = decide_unknown_upload_recovery_v1(
        &started,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(0),
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::ResponseUnknownObservationRequired,
        "window 3: a started request with no response must require observation",
    )?;
    // Window 4: crash after provider acceptance but before the response. The
    // journal is indistinguishable from window 3; observation finds the exact
    // bytes, so the row is skipped and never re-uploaded.
    let exact = observations(
        surface(true, true, Some(true)),
        surface(true, false, None),
        surface(true, false, None),
        surface(true, false, None),
        1,
    );
    let disposition = decide_unknown_upload_recovery_v1(
        &started,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &exact,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::VisibleExactSkipAndContinue,
        "window 4: accepted-but-unrecorded uploads must skip on exact visibility",
    )?;
    // Window 5: response observed but the local journal append fails. The
    // model never sees the lost response, so it decides exactly like window
    // 3: the observed-but-unrecorded response cannot leak into recovery.
    let disposition = decide_unknown_upload_recovery_v1(
        &started,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(0),
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::ResponseUnknownObservationRequired,
        "window 5: unrecorded responses must stay unknown",
    )?;
    // Window 6: local append succeeds (unknown response recorded) but the
    // remote checkpoint fails. The row still needs observation, and no
    // post-observation position exists, so dependants stay blocked.
    let mut unknowned = settled()?;
    ev(
        &mut unknowned,
        Event::UploadRequestStarted,
        Some(row.clone()),
        None,
        None,
        CREATED_AT + 60,
    )?;
    ev(
        &mut unknowned,
        Event::UploadResponseUnknown,
        Some(row.clone()),
        Some(response(UploadResponseClassV1::Timeout)),
        None,
        CREATED_AT + 70,
    )?;
    let disposition = decide_unknown_upload_recovery_v1(
        &unknowned,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(1),
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::ResponseUnknownObservationRequired,
        "window 6: unknown responses without checkpoints must require observation",
    )?;
    // Window 7: API visible while index/download/resolver lag. Visibility
    // the surfaces cannot compare is propagation, never permission.
    let propagating = observations(
        surface(true, true, None),
        surface(true, false, None),
        surface(false, false, None),
        surface(true, false, None),
        2,
    );
    let disposition = decide_unknown_upload_recovery_v1(
        &unknowned,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &propagating,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::WaitingForPropagation,
        "window 7: lagging surfaces must wait for propagation",
    )?;
    // Window 8: all surfaces absent after the bounded wait, verified intent,
    // and bound recovery authority: the original bytes may upload once.
    let position = pre_intent_position(&unknowned)?;
    let disposition = decide_unknown_upload_recovery_v1(
        &unknowned,
        &[position],
        &row,
        &custody(),
        &recovery_authorizations(),
        &absent(MAX_ROUNDS),
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::MissingAfterBoundedObservationRecoveryMayUpload,
        "window 8: bounded absence with intent and authority may upload originals",
    )?;
    // Window 9: provider timeout/rate-limit/malformed surfaces are outages.
    // All dark means stop; partially dark with budget left means observe.
    let outage = observations(
        surface(false, false, None),
        surface(false, false, None),
        surface(false, false, None),
        surface(false, false, None),
        3,
    );
    let disposition = decide_unknown_upload_recovery_v1(
        &unknowned,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &outage,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::ProviderUnavailableStop,
        "window 9: total outage must stop",
    )?;
    // Window 10: exact version visible with a conflicting checksum. Conflict
    // wins over every other surface: incident, never upload.
    let mut conflicted = settled()?;
    ev(
        &mut conflicted,
        Event::UploadRequestStarted,
        Some(row.clone()),
        None,
        None,
        CREATED_AT + 60,
    )?;
    ev(
        &mut conflicted,
        Event::UploadResponseObserved,
        Some(row.clone()),
        Some(response(UploadResponseClassV1::Success)),
        None,
        CREATED_AT + 70,
    )?;
    ev(
        &mut conflicted,
        Event::RegistryObservationStarted,
        Some(row.clone()),
        None,
        None,
        CREATED_AT + 80,
    )?;
    ev(
        &mut conflicted,
        Event::RegistryVisibleConflict,
        Some(row.clone()),
        None,
        Some(PublicationRegistryObservationV1 {
            provider_reachable: true,
            row_visible: true,
            archive_digest_matches: false,
        }),
        CREATED_AT + 90,
    )?;
    ev(
        &mut conflicted,
        Event::OperationIncident,
        None,
        None,
        None,
        CREATED_AT + 100,
    )?;
    let conflict_surfaces = observations(
        surface(true, false, None),
        surface(true, true, Some(false)),
        surface(true, false, None),
        surface(true, false, None),
        2,
    );
    let disposition = decide_unknown_upload_recovery_v1(
        &conflicted,
        &[],
        &row,
        &custody(),
        &recovery_authorizations(),
        &conflict_surfaces,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::VisibleConflictIncident && disposition.preserved_incident,
        "window 10: conflicting bytes must incident with lineage preserved",
    )?;
    // Window 11: the runner restarts twice over the same row. Recovery is a
    // pure function of exact evidence, so identical inputs decide
    // identically: restarts are idempotent, never additive.
    let first = decide_unknown_upload_recovery_v1(
        &unknowned,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(1),
    )
    .map_err(io::Error::other)?;
    let second = decide_unknown_upload_recovery_v1(
        &unknowned,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(1),
    )
    .map_err(io::Error::other)?;
    require(
        first == second,
        "window 11: identical evidence must decide identically across restarts",
    )?;
    // Window 12: a stale or current-main candidate offered for recovery is
    // refused. Recovery continues from original custody bytes only.
    let mut stale = row.clone();
    stale.candidate_archive_digest = digest(99);
    require(
        decide_unknown_upload_recovery_v1(
            &unknowned,
            &[],
            &stale,
            &custody(),
            &recovery_authorizations(),
            &absent(MAX_ROUNDS),
        )
        .is_err(),
        "window 12: stale candidates must fail closed",
    )?;
    // Window 13: clean authorization reused for incident recovery waits for
    // exact recovery authority instead of proceeding on clean.
    let disposition = decide_unknown_upload_recovery_v1(
        &conflicted,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(MAX_ROUNDS),
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::RecoveryAuthorizationRequired,
        "window 13: incident recovery must wait for recovery authority",
    )?;
    // Window 14: all rows already exact. Every row decides skip, so the
    // operation completes with zero uploads and zero new history.
    let mut complete = settled()?;
    ev(
        &mut complete,
        Event::UploadRequestStarted,
        Some(row.clone()),
        None,
        None,
        CREATED_AT + 60,
    )?;
    ev(
        &mut complete,
        Event::UploadResponseObserved,
        Some(row.clone()),
        Some(response(UploadResponseClassV1::Success)),
        None,
        CREATED_AT + 70,
    )?;
    ev(
        &mut complete,
        Event::RegistryObservationStarted,
        Some(row.clone()),
        None,
        None,
        CREATED_AT + 80,
    )?;
    ev(
        &mut complete,
        Event::RegistryVisibleExact,
        Some(row.clone()),
        None,
        Some(exact_observation()),
        CREATED_AT + 90,
    )?;
    let disposition = decide_unknown_upload_recovery_v1(
        &complete,
        &[],
        &row,
        &custody(),
        &authorizations(),
        &absent(0),
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::VisibleExactSkipAndContinue,
        "window 14: reconciled rows must skip with zero uploads",
    )?;
    // Dependants stay blocked until prerequisites are exactly reconciled,
    // even when the dependant row itself never attempted anything.
    let mut chained = begin_journal(vec![
        PublicationJournalRowV1 {
            package_name: "cargo-allow".to_string(),
            version: "0.2.0".to_string(),
            row_order: 0,
            candidate_archive_digest: digest(10),
            depends_on: Vec::new(),
        },
        PublicationJournalRowV1 {
            package_name: "allow-report".to_string(),
            version: "0.2.0".to_string(),
            row_order: 1,
            candidate_archive_digest: digest(11),
            depends_on: vec!["cargo-allow".to_string()],
        },
    ])?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    ev(
        &mut chained,
        Event::OperationSelected,
        None,
        None,
        None,
        next(),
    )?;
    ev(
        &mut chained,
        Event::AuthorizationConsumed,
        None,
        None,
        None,
        next(),
    )?;
    ev(
        &mut chained,
        Event::TagObservedExact,
        None,
        None,
        None,
        next(),
    )?;
    let dependant = match chained
        .rows
        .iter()
        .find(|declared| declared.package_name == "allow-report")
    {
        Some(found) => found.clone(),
        None => return fail("chained fixture lost its dependant row"),
    };
    require(
        recovery_dependant_may_begin_v1(&chained, &dependant).is_err(),
        "dependants must stay blocked while prerequisites wait",
    )?;
    Ok(())
}
