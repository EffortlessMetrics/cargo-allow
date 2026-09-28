//! Authorization separation for unknown-upload recovery (#3924).
//!
//! Synthetic subjects only: no network access, no uploads, no registry
//! observation, no credentials, and nothing leaves the process. Clean
//! authorization starts fresh rows; only exact recovery authority bound to
//! the original candidate may re-upload attempted rows, and only from
//! verified durable intent. These tests prove the separation holds in every
//! direction: clean never substitutes, foreign bindings never satisfy,
//! lineage never clears, dependants never jump the queue, and history is
//! never rewritten by deciding.

use std::error::Error;
use std::io;

use PublicationJournalEventV1 as Event;
use UnknownUploadRecoveryClassV1 as Class;
use allow_report::{
    CargoAllowPublicationJournalV1, PUBLICATION_RECOVERY_MAX_OBSERVATION_ROUNDS as MAX_ROUNDS,
    PublicationCheckpointKindV1, PublicationJournalAppendV1, PublicationJournalClassV1,
    PublicationJournalEventV1, PublicationJournalInitV1, PublicationJournalRowV1,
    RecoveryAuthorizationV1, RecoveryAuthorizationsV1, RecoveryCheckpointPositionV1,
    RecoveryCustodyV1, RecoveryRegistryObservationsV1, RecoverySurfaceObservationV1,
    UnknownUploadRecoveryClassV1, append_journal_event_v1, begin_publication_journal_v1,
    decide_unknown_upload_recovery_v1, recovery_dependant_may_begin_v1,
};

const CREATED_AT: u64 = 1_786_200_000;

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

fn settled() -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    let mut journal = begin_publication_journal_v1(PublicationJournalInitV1 {
        journal_id: "journal-0-2-0-001".to_string(),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_identity_digest: digest(77),
        operation_class: PublicationJournalClassV1::CleanFinalPublication,
        authorization_digest: digest(70),
        custody_digest: digest(71),
        freeze_digest: digest(72),
        prior_journal_digest: None,
        rows: vec![journal_row()],
        created_at_unix_seconds: CREATED_AT,
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "publish".to_string(),
    })
    .map_err(io::Error::other)?;
    let row = journal_row();
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let append = |journal: &mut CargoAllowPublicationJournalV1,
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
    let mut append = append;
    append(&mut journal, Event::OperationSelected, None)?;
    append(&mut journal, Event::AuthorizationConsumed, None)?;
    append(&mut journal, Event::TagObservedExact, None)?;
    append(&mut journal, Event::RowPreflightComplete, Some(row.clone()))?;
    append(&mut journal, Event::UploadIntentDurable, Some(row))?;
    Ok(journal)
}

fn surface(reachable: bool, visible: bool, matches: Option<bool>) -> RecoverySurfaceObservationV1 {
    RecoverySurfaceObservationV1 {
        reachable,
        visible,
        digest_matches: matches,
    }
}

fn absent(rounds: u32) -> RecoveryRegistryObservationsV1 {
    let silent = surface(true, false, None);
    RecoveryRegistryObservationsV1 {
        api: silent,
        index: silent,
        download: silent,
        resolver: silent,
        rounds,
        instrument_failure: false,
    }
}

fn custody() -> RecoveryCustodyV1 {
    RecoveryCustodyV1 {
        candidate_archive_digest: digest(10),
        custody_digest: digest(71),
        freeze_digest: digest(72),
    }
}

fn clean_only() -> RecoveryAuthorizationsV1 {
    RecoveryAuthorizationsV1 {
        clean_authorization_digest: digest(70),
        recovery: None,
    }
}

fn bound_recovery() -> RecoveryAuthorizationsV1 {
    RecoveryAuthorizationsV1 {
        clean_authorization_digest: digest(70),
        recovery: Some(RecoveryAuthorizationV1 {
            authorization_digest: digest(80),
            plan_digest: digest(81),
            bound_candidate_digest: digest(10),
        }),
    }
}

fn attempted() -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    let mut journal = settled()?;
    append_journal_event_v1(
        &mut journal,
        PublicationJournalAppendV1 {
            kind: Event::UploadRequestStarted,
            row: Some(journal_row()),
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 60,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    Ok(journal)
}

fn pre_intent_position(
    journal: &CargoAllowPublicationJournalV1,
) -> Result<RecoveryCheckpointPositionV1, Box<dyn Error>> {
    let intent = match journal.entries.iter().find(|entry| {
        entry.kind == PublicationJournalEventV1::UploadIntentDurable
            && entry.package_name.as_deref() == Some("cargo-allow")
    }) {
        Some(entry) => entry,
        None => return fail("recovery fixtures require the row upload intent"),
    };
    Ok(RecoveryCheckpointPositionV1 {
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_identity_digest: digest(77),
        checkpoint_sequence: 1,
        journal_head_sequence: intent.sequence,
        journal_head_digest: intent.entry_digest.clone(),
        kind: PublicationCheckpointKindV1::PreIntentDurable,
        readback_complete: true,
    })
}

#[test]
fn unknown_upload_recovery_authorization() -> Result<(), Box<dyn Error>> {
    let row = journal_row();
    let journal = attempted()?;
    let position = pre_intent_position(&journal)?;
    let budgeted = absent(MAX_ROUNDS);
    // Clean authorization alone starts fresh rows but never re-uploads
    // attempted ones: the bounded-absent evidence waits for recovery
    // authority instead.
    let waiting = decide_unknown_upload_recovery_v1(
        &journal,
        std::slice::from_ref(&position),
        &row,
        &custody(),
        &clean_only(),
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        waiting.class == Class::RecoveryAuthorizationRequired
            && waiting.requires_recovery_authorization,
        "clean authorization must never re-upload attempted rows",
    )?;
    // Exact recovery authority bound to the original candidate authorizes
    // exactly one re-upload of the original bytes.
    let may_upload = decide_unknown_upload_recovery_v1(
        &journal,
        std::slice::from_ref(&position),
        &row,
        &custody(),
        &bound_recovery(),
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        may_upload.class == Class::MissingAfterBoundedObservationRecoveryMayUpload,
        "bound recovery authority must authorize the original bytes once",
    )?;
    // Only candidate binding is load-bearing: a foreign plan or a foreign
    // authorization digest still authorizes when the candidate binding is
    // exact, while a foreign candidate refuses (covered in
    // release_unknown_upload_recovery.rs).
    let mut foreign_plan = bound_recovery();
    match foreign_plan.recovery.as_mut() {
        Some(recovery) => recovery.plan_digest = digest(99),
        None => return fail("recovery fixture lost its authorization"),
    }
    // The plan digest is carried, not matched: authority binds through the
    // candidate. A foreign plan with an exact candidate binding still
    // authorizes, because the candidate binding is the authority root.
    let plan_foreign = decide_unknown_upload_recovery_v1(
        &journal,
        std::slice::from_ref(&position),
        &row,
        &custody(),
        &foreign_plan,
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        plan_foreign.class == Class::MissingAfterBoundedObservationRecoveryMayUpload,
        "authority binds through the candidate, not the plan label",
    )?;
    let mut foreign_auth = bound_recovery();
    match foreign_auth.recovery.as_mut() {
        Some(recovery) => recovery.authorization_digest = digest(98),
        None => return fail("recovery fixture lost its authorization"),
    }
    let auth_foreign = decide_unknown_upload_recovery_v1(
        &journal,
        std::slice::from_ref(&position),
        &row,
        &custody(),
        &foreign_auth,
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        auth_foreign.class == Class::MissingAfterBoundedObservationRecoveryMayUpload,
        "authority binds through the candidate, not the authorization label",
    )?;
    // Malformed authorization identity fails closed before any disposition.
    let mut malformed = bound_recovery();
    match malformed.recovery.as_mut() {
        Some(recovery) => recovery.authorization_digest = "not-a-digest".to_string(),
        None => return fail("recovery fixture lost its authorization"),
    }
    require(
        decide_unknown_upload_recovery_v1(
            &journal,
            std::slice::from_ref(&position),
            &row,
            &custody(),
            &malformed,
            &budgeted,
        )
        .is_err(),
        "malformed recovery identity must fail closed",
    )?;
    // Malformed clean identity fails closed too: fresh starts need exact
    // clean authority, never an approximate one.
    let mut malformed_clean = clean_only();
    malformed_clean.clean_authorization_digest = String::new();
    require(
        decide_unknown_upload_recovery_v1(
            &settled()?,
            &[],
            &row,
            &custody(),
            &malformed_clean,
            &absent(0),
        )
        .is_err(),
        "malformed clean identity must fail closed",
    )?;
    // Lineage: an incident journal decides with its incident preserved, and
    // deciding never rewrites the journal or the checkpoint positions.
    let mut incident = attempted()?;
    append_journal_event_v1(
        &mut incident,
        PublicationJournalAppendV1 {
            kind: Event::OperationIncident,
            row: None,
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 70,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    let frozen_journal = incident.clone();
    let frozen_position = position.clone();
    let disposition = decide_unknown_upload_recovery_v1(
        &incident,
        &[pre_intent_position(&incident)?],
        &row,
        &custody(),
        &bound_recovery(),
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.preserved_incident,
        "recovery must preserve the incident lineage bit",
    )?;
    require(
        incident == frozen_journal && position == frozen_position,
        "deciding must never rewrite journal or checkpoint history",
    )?;
    // The decided head binds the decision: later history supersedes by
    // reference, and the old disposition keeps pointing at its own head.
    require(
        disposition.decided_journal_head_sequence
            == frozen_journal
                .entries
                .last()
                .map(|entry| entry.sequence)
                .unwrap_or(0),
        "dispositions must bind the head they decided against",
    )?;
    // Dependants stay blocked until prerequisites reconcile exactly, even
    // when every authorization is present and bound.
    let mut chained = begin_publication_journal_v1(allow_report::PublicationJournalInitV1 {
        journal_id: "journal-0-2-0-002".to_string(),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_identity_digest: digest(77),
        operation_class: PublicationJournalClassV1::CleanFinalPublication,
        authorization_digest: digest(70),
        custody_digest: digest(71),
        freeze_digest: digest(72),
        prior_journal_digest: None,
        rows: vec![
            journal_row(),
            PublicationJournalRowV1 {
                package_name: "allow-report".to_string(),
                version: "0.2.0".to_string(),
                row_order: 1,
                candidate_archive_digest: digest(11),
                depends_on: vec!["cargo-allow".to_string()],
            },
        ],
        created_at_unix_seconds: CREATED_AT,
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "publish".to_string(),
    })
    .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let mut append = |journal: &mut CargoAllowPublicationJournalV1,
                      kind: PublicationJournalEventV1,
                      found: Option<PublicationJournalRowV1>| {
        append_journal_event_v1(
            journal,
            PublicationJournalAppendV1 {
                kind,
                row: found,
                response: None,
                observation: None,
                at_unix_seconds: next(),
                reason: "synthetic".to_string(),
            },
        )
        .map_err(io::Error::other)
    };
    append(&mut chained, Event::OperationSelected, None)?;
    append(&mut chained, Event::AuthorizationConsumed, None)?;
    append(&mut chained, Event::TagObservedExact, None)?;
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
    // No failure verdict exists anywhere in the recovery vocabulary: every
    // decided class below is a wait, a skip, a gated upload, a stop, or an
    // instrument report. The exhaustive match means adding a variant breaks
    // compilation until it is classified here.
    for class in [
        Class::NotAttemptedSafeToStart,
        Class::ResponseUnknownObservationRequired,
        Class::VisibleExactSkipAndContinue,
        Class::WaitingForPropagation,
        Class::VisibleConflictIncident,
        Class::MissingAfterBoundedObservationRecoveryMayUpload,
        Class::ProviderUnavailableStop,
        Class::RecoveryAuthorizationRequired,
        Class::InstrumentFailure,
    ] {
        let _disposition = match class {
            Class::NotAttemptedSafeToStart
            | Class::ResponseUnknownObservationRequired
            | Class::WaitingForPropagation
            | Class::RecoveryAuthorizationRequired => "wait",
            Class::VisibleExactSkipAndContinue => "skip",
            Class::MissingAfterBoundedObservationRecoveryMayUpload => "gated-upload",
            Class::VisibleConflictIncident
            | Class::ProviderUnavailableStop
            | Class::InstrumentFailure => "stop-or-report",
        };
        let name = format!("{class:?}");
        require(
            name != "UploadFailed" && name != "UploadFailure" && name != "Failed",
            "the recovery vocabulary must never launder unknown into failure",
        )?;
    }
    Ok(())
}
