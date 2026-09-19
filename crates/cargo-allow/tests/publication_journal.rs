//! Append-only publication journal for the final release (#3921).
//!
//! Synthetic subjects only: no network access, no uploads, no registry
//! observation, no credentials, and nothing leaves the process. These tests
//! prove durable pre-intent per row, honest unknown responses, provider
//! observation verdicts, dependency-ordered preflight, chain integrity, and
//! incident preservation the #2502/#2509 execution lanes will rely on.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use PublicationJournalEventV1 as Event;
use allow_report::{
    CargoAllowPublicationJournalV1, PUBLICATION_JOURNAL_OPERATION,
    PUBLICATION_JOURNAL_RECOVERY_OPERATION, PUBLICATION_JOURNAL_SCHEMA_ID,
    PUBLICATION_JOURNAL_SCHEMA_VERSION, PublicationJournalAppendV1, PublicationJournalClassV1,
    PublicationJournalEventV1, PublicationJournalInitV1, PublicationJournalRowV1,
    PublicationRegistryObservationV1, PublicationUploadResponseV1, UploadResponseClassV1,
    append_journal_event_v1, begin_publication_journal_v1, render_publication_journal_v1,
    verify_publication_journal_v1,
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

fn begin_init_with(rows: Vec<PublicationJournalRowV1>) -> PublicationJournalInitV1 {
    PublicationJournalInitV1 {
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
    }
}

fn begin_init() -> PublicationJournalInitV1 {
    begin_init_with(vec![
        row("cargo-allow", 0, 10, &[]),
        row("allow-report", 1, 11, &["cargo-allow"]),
    ])
}

fn begun() -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    Ok(begin_publication_journal_v1(begin_init()).map_err(io::Error::other)?)
}

fn row(name: &str, order: u32, n: u64, deps: &[&str]) -> PublicationJournalRowV1 {
    PublicationJournalRowV1 {
        package_name: name.to_string(),
        version: "0.2.0".to_string(),
        row_order: order,
        candidate_archive_digest: digest(n),
        depends_on: deps.iter().map(|name| name.to_string()).collect(),
    }
}

fn ev(
    kind: PublicationJournalEventV1,
    row: Option<PublicationJournalRowV1>,
    at: u64,
) -> PublicationJournalAppendV1 {
    PublicationJournalAppendV1 {
        kind,
        row,
        response: None,
        observation: None,
        at_unix_seconds: at,
        reason: "synthetic".to_string(),
    }
}

fn ev_response(
    kind: PublicationJournalEventV1,
    row: PublicationJournalRowV1,
    class: UploadResponseClassV1,
    at: u64,
) -> PublicationJournalAppendV1 {
    PublicationJournalAppendV1 {
        kind,
        row: Some(row),
        response: Some(PublicationUploadResponseV1 {
            class,
            detail: "synthetic position".to_string(),
        }),
        observation: None,
        at_unix_seconds: at,
        reason: "synthetic".to_string(),
    }
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
        None => Err(io::Error::other("journal JSON not an object").into()),
    }
}

fn exact_observation() -> PublicationRegistryObservationV1 {
    PublicationRegistryObservationV1 {
        provider_reachable: true,
        row_visible: true,
        archive_digest_matches: true,
    }
}

fn ev_observed(
    kind: PublicationJournalEventV1,
    row: PublicationJournalRowV1,
    observation: PublicationRegistryObservationV1,
    at: u64,
) -> PublicationJournalAppendV1 {
    PublicationJournalAppendV1 {
        kind,
        row: Some(row),
        response: None,
        observation: Some(observation),
        at_unix_seconds: at,
        reason: "synthetic".to_string(),
    }
}

/// Full clean lifecycle for two ordered rows, then completion.
fn clean_two_rows() -> Result<CargoAllowPublicationJournalV1, Box<dyn Error>> {
    let mut journal = begun()?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let append = |journal: &mut CargoAllowPublicationJournalV1,
                  request: PublicationJournalAppendV1| {
        append_journal_event_v1(journal, request).map_err(io::Error::other)
    };
    append(&mut journal, ev(Event::OperationSelected, None, next()))?;
    append(&mut journal, ev(Event::AuthorizationConsumed, None, next()))?;
    append(&mut journal, ev(Event::TagObservedExact, None, next()))?;
    let first = row("cargo-allow", 0, 10, &[]);
    let second = row("allow-report", 1, 11, &["cargo-allow"]);
    for current in [&first, &second] {
        append(
            &mut journal,
            ev(Event::RowPreflightComplete, Some(current.clone()), next()),
        )?;
        append(
            &mut journal,
            ev(Event::UploadIntentDurable, Some(current.clone()), next()),
        )?;
        append(
            &mut journal,
            ev(Event::UploadRequestStarted, Some(current.clone()), next()),
        )?;
        append(
            &mut journal,
            ev_response(
                Event::UploadResponseObserved,
                current.clone(),
                UploadResponseClassV1::Success,
                next(),
            ),
        )?;
        append(
            &mut journal,
            ev(
                Event::RegistryObservationStarted,
                Some(current.clone()),
                next(),
            ),
        )?;
        append(
            &mut journal,
            ev_observed(
                Event::RegistryVisibleExact,
                current.clone(),
                exact_observation(),
                next(),
            ),
        )?;
    }
    append(&mut journal, ev(Event::OperationComplete, None, next()))?;
    Ok(journal)
}

#[test]
fn publication_journal() -> Result<(), Box<dyn Error>> {
    let journal = clean_two_rows()?;
    require(
        journal.entries.len() == 3 + 2 * 6 + 1,
        "clean two-row lifecycle must append exactly sixteen entries",
    )?;
    require(
        journal
            .entries
            .iter()
            .enumerate()
            .all(|(index, entry)| entry.sequence == index as u64 + 1),
        "journal sequence must be gapless from one",
    )?;
    verify_publication_journal_v1(&journal).map_err(io::Error::other)?;
    // Control: the journal records the exact custody-bound archive digest it
    // was given; fidelity here is what lets #2502 bind custody bytes.
    let recorded = journal
        .entries
        .iter()
        .find(|entry| {
            entry.kind == Event::UploadIntentDurable
                && entry.package_name.as_deref() == Some("cargo-allow")
        })
        .ok_or_else(|| io::Error::other("durable intent entry absent"))?;
    require(
        recorded.candidate_archive_digest.as_deref() == Some(digest(10).as_str()),
        "the journal must record the supplied candidate digest verbatim",
    )?;
    // Control: the rendered journal validates against its schema.
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.publication-journal.v1.schema.json"),
        )?)?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_publication_journal_v1(&journal)?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("journal schema compiles: {error}")))?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!("rendered journal violates schema: {error}"))
        })?;
        require(
            rendered.get("schema_id")
                == Some(&serde_json::Value::String(
                    PUBLICATION_JOURNAL_SCHEMA_ID.into(),
                ))
                && rendered.get("schema_version")
                    == Some(&serde_json::Value::from(PUBLICATION_JOURNAL_SCHEMA_VERSION)),
            "rendered journal must carry its schema identity",
        )?;
    }
    Ok(())
}

#[test]
fn publication_journal_faults() -> Result<(), Box<dyn Error>> {
    // Control: operation events carry no row; row events require one.
    let mut journal = begun()?;
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::AuthorizationConsumed, None, CREATED_AT + 10),
        )
        .is_err(),
        "consumption before selection must fail",
    )?;
    require(
        append_journal_event_v1(
            &mut journal,
            ev(
                Event::OperationSelected,
                Some(row("cargo-allow", 0, 10, &[])),
                CREATED_AT + 10,
            ),
        )
        .is_err(),
        "operation selection with a row must fail",
    )?;
    append_journal_event_v1(
        &mut journal,
        ev(Event::OperationSelected, None, CREATED_AT + 10),
    )
    .map_err(io::Error::other)?;
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::OperationSelected, None, CREATED_AT + 20),
        )
        .is_err(),
        "a second operation selection must fail",
    )?;
    // Control: identity values are canonical lowercase hex.
    let mut init = begin_init();
    init.authorization_digest = format!("sha256:{}", "A".repeat(64));
    require(
        begin_publication_journal_v1(init).is_err(),
        "uppercase identity hex must fail closed",
    )?;
    // Control: row preflight waits for exact dependency visibility.
    append_journal_event_v1(
        &mut journal,
        ev(Event::AuthorizationConsumed, None, CREATED_AT + 20),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut journal,
        ev(Event::TagObservedExact, None, CREATED_AT + 30),
    )
    .map_err(io::Error::other)?;
    let second = row("allow-report", 1, 11, &["cargo-allow"]);
    require(
        append_journal_event_v1(
            &mut journal,
            ev(
                Event::RowPreflightComplete,
                Some(second.clone()),
                CREATED_AT + 40,
            ),
        )
        .is_err(),
        "dependent preflight before dependency visibility must fail",
    )?;
    // Control: rows must repeat the bound declaration exactly; a swapped
    // archive digest cannot enter mid-operation.
    let mut swapped = row("cargo-allow", 0, 10, &[]);
    swapped.candidate_archive_digest = digest(99);
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::RowPreflightComplete, Some(swapped), CREATED_AT + 45),
        )
        .is_err(),
        "preflight with a swapped candidate digest must fail",
    )?;
    // Control: intent is durable before the request; responses belong to
    // started uploads; unknown is recorded, never inferred.
    let first = row("cargo-allow", 0, 10, &[]);
    append_journal_event_v1(
        &mut journal,
        ev(
            Event::RowPreflightComplete,
            Some(first.clone()),
            CREATED_AT + 40,
        ),
    )
    .map_err(io::Error::other)?;
    require(
        append_journal_event_v1(
            &mut journal,
            ev(
                Event::UploadRequestStarted,
                Some(first.clone()),
                CREATED_AT + 50,
            ),
        )
        .is_err(),
        "requests without durable intent must fail",
    )?;
    append_journal_event_v1(
        &mut journal,
        ev(
            Event::UploadIntentDurable,
            Some(first.clone()),
            CREATED_AT + 50,
        ),
    )
    .map_err(io::Error::other)?;
    require(
        append_journal_event_v1(
            &mut journal,
            ev_response(
                Event::UploadResponseUnknown,
                first.clone(),
                UploadResponseClassV1::Timeout,
                CREATED_AT + 60,
            ),
        )
        .is_err(),
        "responses without a started request must fail",
    )?;
    append_journal_event_v1(
        &mut journal,
        ev(
            Event::UploadRequestStarted,
            Some(first.clone()),
            CREATED_AT + 60,
        ),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut journal,
        ev_response(
            Event::UploadResponseUnknown,
            first.clone(),
            UploadResponseClassV1::Timeout,
            CREATED_AT + 70,
        ),
    )
    .map_err(io::Error::other)?;
    require(
        journal.entries.last().map(|entry| entry.kind) == Some(Event::UploadResponseUnknown),
        "a lost response must persist as unknown",
    )?;
    // Control: a nonzero response reconciled by later exact visibility.
    append_journal_event_v1(
        &mut journal,
        ev(
            Event::RegistryObservationStarted,
            Some(first.clone()),
            CREATED_AT + 80,
        ),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut journal,
        ev_observed(
            Event::RegistryVisibleExact,
            first.clone(),
            exact_observation(),
            CREATED_AT + 90,
        ),
    )
    .map_err(io::Error::other)?;
    // Control: digest mismatches are conflicts, never exact.
    let mut mismatch = exact_observation();
    mismatch.archive_digest_matches = false;
    require(
        append_journal_event_v1(
            &mut journal,
            ev_observed(
                Event::RegistryVisibleExact,
                second.clone(),
                mismatch,
                CREATED_AT + 100,
            ),
        )
        .is_err(),
        "a digest mismatch must refuse exact visibility",
    )?;
    // Control: completion requires every intent row terminally resolved.
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::OperationComplete, None, CREATED_AT + 100),
        )
        .is_err(),
        "completion with an unstarted dependent row must fail",
    )?;
    // Control: unbounded provider notes must fail, and secret markers in
    // reasons must fail closed under the shared custody marker law.
    let mut oversized = ev_response(
        Event::UploadResponseObserved,
        second.clone(),
        UploadResponseClassV1::Success,
        CREATED_AT + 100,
    );
    oversized.response = Some(PublicationUploadResponseV1 {
        class: UploadResponseClassV1::Success,
        detail: "x".repeat(257),
    });
    require(
        append_journal_event_v1(&mut journal, oversized).is_err(),
        "unbounded provider notes must fail",
    )?;
    let mut leaked = ev(Event::OperationIncident, None, CREATED_AT + 100);
    leaked.reason = "retry token=synthetic-secret".to_string();
    require(
        append_journal_event_v1(&mut journal, leaked).is_err(),
        "secret material in a journal reason must fail",
    )?;
    // Control: entries are append-only; edits break verification.
    let mut tampered = clean_two_rows()?;
    tampered
        .entries
        .get_mut(3)
        .ok_or_else(|| io::Error::other("journal entry absent"))?
        .reason = "rewritten".to_string();
    require(
        verify_publication_journal_v1(&tampered).is_err(),
        "an edited entry must break chain verification",
    )?;
    let mut reordered = clean_two_rows()?;
    reordered.entries.swap(0, 1);
    require(
        verify_publication_journal_v1(&reordered).is_err(),
        "a reordered journal must break chain verification",
    )?;
    // Control: a proven absence after an unknown response authorizes one
    // careful re-invocation; the attempt bound stops retry loops.
    let mut journal =
        begin_publication_journal_v1(begin_init_with(vec![row("cargo-allow", 0, 10, &[])]))
            .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let append = |journal: &mut CargoAllowPublicationJournalV1,
                  request: PublicationJournalAppendV1| {
        append_journal_event_v1(journal, request).map_err(io::Error::other)
    };
    append(&mut journal, ev(Event::OperationSelected, None, next()))?;
    append(&mut journal, ev(Event::AuthorizationConsumed, None, next()))?;
    append(&mut journal, ev(Event::TagObservedExact, None, next()))?;
    let retry_row = row("cargo-allow", 0, 10, &[]);
    append(
        &mut journal,
        ev(Event::RowPreflightComplete, Some(retry_row.clone()), next()),
    )?;
    for _ in 0..3 {
        append(
            &mut journal,
            ev(Event::UploadIntentDurable, Some(retry_row.clone()), next()),
        )?;
        append(
            &mut journal,
            ev(Event::UploadRequestStarted, Some(retry_row.clone()), next()),
        )?;
        append(
            &mut journal,
            ev_response(
                Event::UploadResponseUnknown,
                retry_row.clone(),
                UploadResponseClassV1::Timeout,
                next(),
            ),
        )?;
        append(
            &mut journal,
            ev(
                Event::RegistryObservationStarted,
                Some(retry_row.clone()),
                next(),
            ),
        )?;
        append(
            &mut journal,
            ev_observed(
                Event::RegistryObservedAbsent,
                retry_row.clone(),
                PublicationRegistryObservationV1 {
                    provider_reachable: true,
                    row_visible: false,
                    archive_digest_matches: false,
                },
                next(),
            ),
        )?;
    }
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::UploadIntentDurable, Some(retry_row.clone()), next()),
        )
        .is_err(),
        "a fourth upload attempt must fail for operator decision",
    )?;
    // Control: retry from a waiting row without proven absence must fail.
    let mut waiting =
        begin_publication_journal_v1(begin_init_with(vec![row("cargo-allow", 0, 10, &[])]))
            .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let append = |journal: &mut CargoAllowPublicationJournalV1,
                  request: PublicationJournalAppendV1| {
        append_journal_event_v1(journal, request).map_err(io::Error::other)
    };
    append(&mut waiting, ev(Event::OperationSelected, None, next()))?;
    append(&mut waiting, ev(Event::AuthorizationConsumed, None, next()))?;
    append(&mut waiting, ev(Event::TagObservedExact, None, next()))?;
    let waiting_row = row("cargo-allow", 0, 10, &[]);
    append(
        &mut waiting,
        ev(
            Event::RowPreflightComplete,
            Some(waiting_row.clone()),
            next(),
        ),
    )?;
    append(
        &mut waiting,
        ev(
            Event::UploadIntentDurable,
            Some(waiting_row.clone()),
            next(),
        ),
    )?;
    append(
        &mut waiting,
        ev(
            Event::UploadRequestStarted,
            Some(waiting_row.clone()),
            next(),
        ),
    )?;
    append(
        &mut waiting,
        ev_response(
            Event::UploadResponseUnknown,
            waiting_row.clone(),
            UploadResponseClassV1::Timeout,
            next(),
        ),
    )?;
    append(
        &mut waiting,
        ev(
            Event::RegistryObservationStarted,
            Some(waiting_row.clone()),
            next(),
        ),
    )?;
    append(
        &mut waiting,
        ev(Event::RegistryWaiting, Some(waiting_row.clone()), next()),
    )?;
    require(
        append_journal_event_v1(
            &mut waiting,
            ev(
                Event::UploadIntentDurable,
                Some(waiting_row.clone()),
                next(),
            ),
        )
        .is_err(),
        "re-invocation from an unproven waiting row must fail",
    )?;
    // Control: recovery journals bind the original; clean journals cannot.
    let mut recovery = begin_init();
    recovery.operation_class = PublicationJournalClassV1::IncidentRecovery;
    require(
        begin_publication_journal_v1(recovery).is_err(),
        "recovery without a prior journal digest must fail",
    )?;
    let mut clean = begin_init();
    clean.prior_journal_digest = Some(digest(99));
    require(
        begin_publication_journal_v1(clean).is_err(),
        "a clean journal must not bind a prior journal",
    )?;
    Ok(())
}

#[test]
fn publication_journal_incident_preservation() -> Result<(), Box<dyn Error>> {
    let mut journal =
        begin_publication_journal_v1(begin_init_with(vec![row("cargo-allow", 0, 10, &[])]))
            .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let append = |journal: &mut CargoAllowPublicationJournalV1,
                  request: PublicationJournalAppendV1| {
        append_journal_event_v1(journal, request).map_err(io::Error::other)
    };
    append(&mut journal, ev(Event::OperationSelected, None, next()))?;
    append(&mut journal, ev(Event::AuthorizationConsumed, None, next()))?;
    append(&mut journal, ev(Event::TagObservedExact, None, next()))?;
    let first_row = row("cargo-allow", 0, 10, &[]);
    append(
        &mut journal,
        ev(Event::RowPreflightComplete, Some(first_row.clone()), next()),
    )?;
    append(
        &mut journal,
        ev(Event::UploadIntentDurable, Some(first_row.clone()), next()),
    )?;
    append(
        &mut journal,
        ev(Event::UploadRequestStarted, Some(first_row.clone()), next()),
    )?;
    append(
        &mut journal,
        ev_response(
            Event::UploadResponseObserved,
            first_row.clone(),
            UploadResponseClassV1::NonzeroExit,
            next(),
        ),
    )?;
    append(
        &mut journal,
        ev(
            Event::RegistryObservationStarted,
            Some(first_row.clone()),
            next(),
        ),
    )?;
    append(
        &mut journal,
        ev_observed(
            Event::RegistryVisibleConflict,
            first_row.clone(),
            PublicationRegistryObservationV1 {
                provider_reachable: true,
                row_visible: true,
                archive_digest_matches: false,
            },
            next(),
        ),
    )?;
    // Control: completion with a conflict and no incident must fail, so a
    // later aggregate cannot silently omit the incident.
    require(
        append_journal_event_v1(&mut journal, ev(Event::OperationComplete, None, next())).is_err(),
        "completion over an unrecorded conflict must fail",
    )?;
    append(&mut journal, ev(Event::OperationIncident, None, next()))?;
    // Control: clean authorization cannot continue after the incident;
    // recovery starts a new journal bound to this one.
    let later = row("allow-report", 1, 11, &[]);
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::UploadIntentDurable, Some(later), next()),
        )
        .is_err(),
        "upload intent after an incident must fail on the clean journal",
    )?;
    append(&mut journal, ev(Event::OperationComplete, None, next()))?;
    verify_publication_journal_v1(&journal).map_err(io::Error::other)?;
    require(
        journal
            .entries
            .iter()
            .any(|entry| entry.kind == Event::RegistryVisibleConflict)
            && journal
                .entries
                .iter()
                .any(|entry| entry.kind == Event::OperationIncident),
        "the completed journal must retain the conflict and the incident",
    )?;
    // Control: the recovery journal binds the original head digest.
    let head = journal
        .entries
        .last()
        .ok_or_else(|| io::Error::other("journal head absent"))?
        .entry_digest
        .clone();
    let mut recovery_init = begin_init();
    recovery_init.journal_id = "journal-0-2-0-002".to_string();
    recovery_init.operation_class = PublicationJournalClassV1::IncidentRecovery;
    recovery_init.operation_id = PUBLICATION_JOURNAL_RECOVERY_OPERATION.to_string();
    recovery_init.prior_journal_digest = Some(head.clone());
    let recovery = begin_publication_journal_v1(recovery_init).map_err(io::Error::other)?;
    require(
        recovery.prior_journal_digest.as_deref() == Some(head.as_str()),
        "recovery must bind the original journal head",
    )?;
    Ok(())
}

#[test]
fn publication_journal_review_repairs() -> Result<(), Box<dyn Error>> {
    // Hostile: depends_on names outside the bound row set must fail at init.
    require(
        begin_publication_journal_v1(begin_init_with(vec![row(
            "cargo-allow",
            0,
            10,
            &["absent-crate"],
        )]))
        .is_err(),
        "external depends_on must fail closed at construction",
    )?;
    // Hostile: operation_id must match its class exactly.
    let mut clean_wrong = begin_init();
    clean_wrong.operation_id = PUBLICATION_JOURNAL_RECOVERY_OPERATION.to_string();
    require(
        begin_publication_journal_v1(clean_wrong).is_err(),
        "clean journals must refuse the recovery operation id",
    )?;
    let mut recovery_wrong = begin_init();
    recovery_wrong.operation_class = PublicationJournalClassV1::IncidentRecovery;
    recovery_wrong.operation_id = PUBLICATION_JOURNAL_OPERATION.to_string();
    recovery_wrong.prior_journal_digest = Some(digest(99));
    require(
        begin_publication_journal_v1(recovery_wrong).is_err(),
        "recovery journals must refuse the clean operation id",
    )?;
    let mut recovery_ok = begin_init();
    recovery_ok.operation_class = PublicationJournalClassV1::IncidentRecovery;
    recovery_ok.operation_id = PUBLICATION_JOURNAL_RECOVERY_OPERATION.to_string();
    recovery_ok.prior_journal_digest = Some(digest(99));
    begin_publication_journal_v1(recovery_ok).map_err(io::Error::other)?;

    // Hostile: a not-yet-started row cannot start after an incident, while a
    // response for an already-started upload stays recordable.
    let mut journal =
        begin_publication_journal_v1(begin_init_with(vec![row("cargo-allow", 0, 10, &[])]))
            .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let first = row("cargo-allow", 0, 10, &[]);
    append_journal_event_v1(&mut journal, ev(Event::OperationSelected, None, next()))
        .map_err(io::Error::other)?;
    append_journal_event_v1(&mut journal, ev(Event::AuthorizationConsumed, None, next()))
        .map_err(io::Error::other)?;
    append_journal_event_v1(&mut journal, ev(Event::TagObservedExact, None, next()))
        .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut journal,
        ev(Event::RowPreflightComplete, Some(first.clone()), next()),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut journal,
        ev(Event::UploadIntentDurable, Some(first.clone()), next()),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut journal,
        ev(Event::UploadRequestStarted, Some(first.clone()), next()),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(&mut journal, ev(Event::OperationIncident, None, next()))
        .map_err(io::Error::other)?;
    // The started row may still record its observed response.
    append_journal_event_v1(
        &mut journal,
        ev_response(
            Event::UploadResponseObserved,
            first.clone(),
            UploadResponseClassV1::TransportError,
            next(),
        ),
    )
    .map_err(io::Error::other)?;
    // A fresh row (or a second request on the same row) must not start.
    require(
        append_journal_event_v1(
            &mut journal,
            ev(Event::UploadRequestStarted, Some(first.clone()), next()),
        )
        .is_err(),
        "upload requests must not start after an incident",
    )?;

    // Hostile: conflict verdicts require reachable + visible + mismatched.
    let mut conflict =
        begin_publication_journal_v1(begin_init_with(vec![row("cargo-allow", 0, 10, &[])]))
            .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    let row0 = row("cargo-allow", 0, 10, &[]);
    for kind in [
        Event::OperationSelected,
        Event::AuthorizationConsumed,
        Event::TagObservedExact,
    ] {
        append_journal_event_v1(&mut conflict, ev(kind, None, next())).map_err(io::Error::other)?;
    }
    append_journal_event_v1(
        &mut conflict,
        ev(Event::RowPreflightComplete, Some(row0.clone()), next()),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut conflict,
        ev(Event::UploadIntentDurable, Some(row0.clone()), next()),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut conflict,
        ev(Event::UploadRequestStarted, Some(row0.clone()), next()),
    )
    .map_err(io::Error::other)?;
    append_journal_event_v1(
        &mut conflict,
        ev_response(
            Event::UploadResponseObserved,
            row0.clone(),
            UploadResponseClassV1::Success,
            next(),
        ),
    )
    .map_err(io::Error::other)?;
    for bad in [
        PublicationRegistryObservationV1 {
            provider_reachable: false,
            row_visible: true,
            archive_digest_matches: false,
        },
        PublicationRegistryObservationV1 {
            provider_reachable: true,
            row_visible: false,
            archive_digest_matches: false,
        },
        PublicationRegistryObservationV1 {
            provider_reachable: true,
            row_visible: true,
            archive_digest_matches: true,
        },
    ] {
        require(
            append_journal_event_v1(
                &mut conflict,
                ev_observed(Event::RegistryVisibleConflict, row0.clone(), bad, next()),
            )
            .is_err(),
            "non-conflicting observations must refuse conflict verdicts",
        )?;
    }

    // Hostile: completion is once-only and terminal.
    let mut done = clean_two_rows()?;
    let head_at = done
        .entries
        .last()
        .map(|entry| entry.at_unix_seconds)
        .unwrap_or(CREATED_AT)
        + 10;
    require(
        append_journal_event_v1(&mut done, ev(Event::OperationComplete, None, head_at)).is_err(),
        "a second completion must fail",
    )?;
    require(
        append_journal_event_v1(&mut done, ev(Event::OperationIncident, None, head_at)).is_err(),
        "incidents after completion must fail",
    )?;
    require(
        append_journal_event_v1(
            &mut done,
            ev(
                Event::RowPreflightComplete,
                Some(row("cargo-allow", 0, 10, &[])),
                head_at,
            ),
        )
        .is_err(),
        "appends after completion must fail",
    )?;

    // Hostile: header and denominator mutation must break verification.
    let clean = clean_two_rows()?;
    let mut mutated_header = clean.clone();
    mutated_header.freeze_digest = digest(999);
    require(
        verify_publication_journal_v1(&mutated_header).is_err(),
        "freeze digest mutation must break verification",
    )?;
    let mut mutated_operation = clean.clone();
    mutated_operation.operation_id = PUBLICATION_JOURNAL_RECOVERY_OPERATION.to_string();
    require(
        verify_publication_journal_v1(&mutated_operation).is_err(),
        "operation identity mutation must break verification",
    )?;
    let mut mutated_prior = clean.clone();
    mutated_prior.prior_journal_digest = Some(digest(99));
    require(
        verify_publication_journal_v1(&mutated_prior).is_err(),
        "prior journal mutation must break verification",
    )?;
    let mut mutated_rows = clean.clone();
    mutated_rows
        .rows
        .get_mut(0)
        .ok_or_else(|| io::Error::other("bound row absent"))?
        .candidate_archive_digest = digest(999);
    require(
        verify_publication_journal_v1(&mutated_rows).is_err(),
        "bound row set mutation must break verification",
    )?;
    // Control: copying entries into a foreign journal identity must fail.
    let mut foreign_init = begin_init();
    foreign_init.journal_id = "journal-foreign".to_string();
    let mut foreign = begin_publication_journal_v1(foreign_init).map_err(io::Error::other)?;
    foreign.entries = clean.entries.clone();
    require(
        verify_publication_journal_v1(&foreign).is_err(),
        "entries copied into a foreign journal must fail verification",
    )?;

    // Hostile: schema enforces the same prior-digest law as Rust.
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.publication-journal.v1.schema.json"),
        )?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("journal schema compiles: {error}")))?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_publication_journal_v1(&clean_two_rows()?)?)?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!(
                "clean journal with null prior must validate: {error}"
            ))
        })?;
        // Clean with a bound prior digest must fail.
        let mut clean_bound = rendered.clone();
        set_field(
            &mut clean_bound,
            "prior_journal_digest",
            serde_json::Value::String(format!("sha256:{:064x}", 99)),
        )?;
        require(
            validator.validate(&clean_bound).is_err(),
            "clean journals must not bind a prior digest in schema",
        )?;
        // Recovery with null prior must fail; with a digest must pass.
        let mut recovery_null = rendered.clone();
        set_field(
            &mut recovery_null,
            "operation_class",
            serde_json::Value::String("incident_recovery".into()),
        )?;
        set_field(
            &mut recovery_null,
            "prior_journal_digest",
            serde_json::Value::Null,
        )?;
        require(
            validator.validate(&recovery_null).is_err(),
            "recovery journals require a prior digest in schema",
        )?;
        let mut recovery_bound = rendered.clone();
        set_field(
            &mut recovery_bound,
            "operation_class",
            serde_json::Value::String("incident_recovery".into()),
        )?;
        set_field(
            &mut recovery_bound,
            "prior_journal_digest",
            serde_json::Value::String(format!("sha256:{:064x}", 99)),
        )?;
        validator.validate(&recovery_bound).map_err(|error| {
            io::Error::other(format!("bound recovery journal must validate: {error}"))
        })?;
        // Missing field must fail for both classes.
        let mut missing = rendered.clone();
        missing
            .as_object_mut()
            .ok_or_else(|| io::Error::other("journal JSON not an object"))?
            .remove("prior_journal_digest");
        require(
            validator.validate(&missing).is_err(),
            "missing prior_journal_digest must fail schema",
        )?;
    }
    Ok(())
}
