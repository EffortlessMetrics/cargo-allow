//! Deterministic unknown-upload recovery decisions (#3924).
//!
//! Synthetic subjects only: no network access, no uploads, no registry
//! observation, no credentials, and nothing leaves the process. Provider
//! observations are caller-supplied multi-surface claims; the decision
//! classifies them but never fetches them. These tests prove every result
//! class of the production recovery state machine: unknown stays unknown,
//! exact means skip, conflict means incident, absence after the bounded
//! budget gates on verified intent plus exact recovery authority, outage is
//! never absence, and clean authorization never substitutes for recovery
//! authority.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use UnknownUploadRecoveryClassV1 as Class;
use allow_report::{
    CargoAllowPublicationJournalV1, CargoAllowUnknownUploadRecoveryV1,
    PUBLICATION_RECOVERY_MAX_OBSERVATION_ROUNDS, PUBLICATION_RECOVERY_SCHEMA_ID,
    PUBLICATION_RECOVERY_SCHEMA_VERSION, PublicationCheckpointKindV1, PublicationJournalAppendV1,
    PublicationJournalClassV1, PublicationJournalEventV1, PublicationJournalInitV1,
    PublicationJournalRowV1, RecoveryAuthorizationV1, RecoveryAuthorizationsV1,
    RecoveryCheckpointPositionV1, RecoveryCustodyV1, RecoveryRegistryObservationsV1,
    RecoverySurfaceObservationV1, UnknownUploadRecoveryClassV1, append_journal_event_v1,
    begin_publication_journal_v1, decide_unknown_upload_recovery_v1,
    render_unknown_upload_recovery_v1,
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

fn journal_row() -> PublicationJournalRowV1 {
    PublicationJournalRowV1 {
        package_name: "cargo-allow".to_string(),
        version: "0.2.0".to_string(),
        row_order: 0,
        candidate_archive_digest: digest(10),
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
                  request: PublicationJournalAppendV1| {
        append_journal_event_v1(journal, request).map_err(io::Error::other)
    };
    let ev = |kind: PublicationJournalEventV1, row: Option<PublicationJournalRowV1>, at: u64| {
        PublicationJournalAppendV1 {
            kind,
            row,
            response: None,
            observation: None,
            at_unix_seconds: at,
            reason: "synthetic".to_string(),
        }
    };
    append(
        &mut journal,
        ev(PublicationJournalEventV1::OperationSelected, None, next()),
    )?;
    append(
        &mut journal,
        ev(
            PublicationJournalEventV1::AuthorizationConsumed,
            None,
            next(),
        ),
    )?;
    append(
        &mut journal,
        ev(PublicationJournalEventV1::TagObservedExact, None, next()),
    )?;
    append(
        &mut journal,
        ev(
            PublicationJournalEventV1::RowPreflightComplete,
            Some(row.clone()),
            next(),
        ),
    )?;
    append(
        &mut journal,
        ev(
            PublicationJournalEventV1::UploadIntentDurable,
            Some(row),
            next(),
        ),
    )?;
    Ok(journal)
}

fn append_started(
    journal: &mut CargoAllowPublicationJournalV1,
    at: u64,
) -> Result<(), Box<dyn Error>> {
    append_journal_event_v1(
        journal,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::UploadRequestStarted,
            row: Some(journal_row()),
            response: None,
            observation: None,
            at_unix_seconds: at,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    Ok(())
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

fn decide(
    journal: &CargoAllowPublicationJournalV1,
    checkpoints: &[RecoveryCheckpointPositionV1],
    observations: RecoveryRegistryObservationsV1,
) -> Result<CargoAllowUnknownUploadRecoveryV1, Box<dyn Error>> {
    Ok(decide_unknown_upload_recovery_v1(
        journal,
        checkpoints,
        &journal_row(),
        &custody(),
        &authorizations(),
        &observations,
    )
    .map_err(io::Error::other)?)
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
        None => fail("recovery JSON not an object"),
    }
}

#[test]
fn unknown_upload_recovery() -> Result<(), Box<dyn Error>> {
    // A never-attempted row with a clean journal starts fresh under clean
    // authorization: no recovery authority is consumed or required.
    let journal = settled_journal()?;
    let disposition = decide(&journal, &[], absent(0))?;
    require(
        disposition.class == Class::NotAttemptedSafeToStart
            && !disposition.requires_recovery_authorization
            && !disposition.preserved_incident,
        "a never-attempted clean row must be safe to start",
    )?;
    // A started upload with no surface evidence needs observation rounds,
    // not a verdict: unknown stays unknown.
    let mut started = settled_journal()?;
    append_started(&mut started, CREATED_AT + 60)?;
    let disposition = decide(&started, &[], absent(0))?;
    require(
        disposition.class == Class::ResponseUnknownObservationRequired,
        "a started upload with no evidence must require observation",
    )?;
    // A reachable surface that cannot compare digests yet is propagation in
    // flight: wait, never infer.
    let waiting = observations(
        surface(true, true, None),
        surface(true, false, None),
        surface(true, false, None),
        surface(true, false, None),
        2,
    );
    let disposition = decide(&started, &[], waiting)?;
    require(
        disposition.class == Class::WaitingForPropagation,
        "uncompared visibility must wait for propagation",
    )?;
    // Checksum equality on any reachable surface is exact: skip, never
    // re-upload — even while other surfaces lag.
    let exact = observations(
        surface(true, true, Some(true)),
        surface(true, false, None),
        surface(false, false, None),
        surface(true, false, None),
        2,
    );
    let disposition = decide(&started, &[], exact)?;
    require(
        disposition.class == Class::VisibleExactSkipAndContinue
            && !disposition.requires_recovery_authorization,
        "checksum equality must skip without re-upload",
    )?;
    // Any visible mismatch is an incident, even beside an agreeing surface.
    let conflict = observations(
        surface(true, true, Some(true)),
        surface(true, true, Some(false)),
        surface(true, false, None),
        surface(true, false, None),
        2,
    );
    let disposition = decide(&started, &[], conflict)?;
    require(
        disposition.class == Class::VisibleConflictIncident,
        "any visible digest mismatch must be an incident",
    )?;
    // Total provider silence stops the lane: outage is never absence.
    let dark = observations(
        surface(false, false, None),
        surface(false, false, None),
        surface(false, false, None),
        surface(false, false, None),
        2,
    );
    let disposition = decide(&started, &[], dark)?;
    require(
        disposition.class == Class::ProviderUnavailableStop,
        "total provider silence must stop",
    )?;
    // A failed observation harness reports instrument failure, never a
    // recovery verdict.
    let mut broken = absent(2);
    broken.instrument_failure = true;
    let disposition = decide(&started, &[], broken)?;
    require(
        disposition.class == Class::InstrumentFailure,
        "harness failure must report instrument failure",
    )?;
    // The bounded budget spent with every surface answering absent, a
    // verified pre-intent position, and exact recovery authority bound to
    // the original candidate: the original bytes may upload exactly once.
    let position = pre_intent_position(&started)?;
    let budgeted = absent(PUBLICATION_RECOVERY_MAX_OBSERVATION_ROUNDS);
    let disposition = decide_unknown_upload_recovery_v1(
        &started,
        &[position],
        &journal_row(),
        &custody(),
        &recovery_authorizations(),
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::MissingAfterBoundedObservationRecoveryMayUpload
            && disposition.requires_recovery_authorization,
        "bounded absence with verified intent and bound recovery authority may upload",
    )?;
    // Same evidence without recovery authority waits for it: clean
    // authorization never substitutes.
    let disposition = decide(&started, &[pre_intent_position(&started)?], budgeted)?;
    require(
        disposition.class == Class::RecoveryAuthorizationRequired
            && disposition.requires_recovery_authorization,
        "bounded absence without recovery authority must wait for it",
    )?;
    // Recovery authority bound to another candidate is not authority here.
    let mut foreign = recovery_authorizations();
    match foreign.recovery.as_mut() {
        Some(recovery) => recovery.bound_candidate_digest = digest(99),
        None => return fail("recovery fixture lost its authorization"),
    }
    let disposition = decide_unknown_upload_recovery_v1(
        &started,
        &[pre_intent_position(&started)?],
        &journal_row(),
        &custody(),
        &foreign,
        &budgeted,
    )
    .map_err(io::Error::other)?;
    require(
        disposition.class == Class::RecoveryAuthorizationRequired,
        "foreign-bound recovery authority must wait for exact authority",
    )?;
    // A stale candidate offered for recovery is refused: only original
    // custody bytes continue.
    let mut stale_row = journal_row();
    stale_row.candidate_archive_digest = digest(99);
    require(
        decide_unknown_upload_recovery_v1(
            &started,
            &[],
            &stale_row,
            &custody(),
            &authorizations(),
            &absent(0),
        )
        .is_err(),
        "a stale candidate must fail closed",
    )?;
    // Control: the rendered disposition validates against its schema, and a
    // foreign class value does not.
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.publication-recovery.v1.schema.json"),
        )?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("recovery schema compiles: {error}")))?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_unknown_upload_recovery_v1(&disposition)?)?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!("rendered disposition must validate: {error}"))
        })?;
        require(
            rendered.get("schema_id")
                == Some(&serde_json::Value::String(
                    PUBLICATION_RECOVERY_SCHEMA_ID.into(),
                ))
                && rendered.get("schema_version")
                    == Some(&serde_json::Value::from(
                        PUBLICATION_RECOVERY_SCHEMA_VERSION,
                    )),
            "rendered disposition must carry its schema identity",
        )?;
        let mut foreign_class = rendered.clone();
        set_field(
            &mut foreign_class,
            "class",
            serde_json::Value::String("upload_failed".into()),
        )?;
        require(
            validator.validate(&foreign_class).is_err(),
            "no failure verdict exists in the recovery vocabulary",
        )?;
    }
    Ok(())
}
