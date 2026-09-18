//! Append-only publication journal for the final release (#3921).
//!
//! #2502 publishes one exact operation across many package rows with
//! crash-prone seams between them: an upload may be accepted by crates.io
//! while the runner dies before recording the response. An aggregate written
//! after the loop cannot distinguish "never attempted" from "accepted but
//! unrecorded", which invites duplicate uploads, unsafe recovery, or a later
//! green run hiding the first irreversible event.
//!
//! This module gives every package row one durable pre-intent and one
//! append-only outcome history under a single operation identity. Responses
//! are recorded as observed or unknown, never inferred; registry visibility
//! comes from caller-supplied provider observations, never process narrative;
//! entries are never edited (correction is another entry); a later success
//! cannot rewrite or omit an earlier incident. After an incident the clean
//! journal refuses further upload intent: recovery starts a new journal bound
//! to this one under #2509.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! uploads, no credential reads, no registry observation fetching, and no
//! live-state mutation. Observations are caller-supplied; the journal
//! validates them but never fetches them. The #2502 execution lane appends
//! entries around the real publisher calls and must treat an append failure
//! as a stop signal: ignoring a logging failure and uploading anyway would
//! recreate exactly the ambiguity this journal exists to remove.

use serde::{Deserialize, Serialize};

use super::release_authorization_custody_v1::secret_marker;

pub const PUBLICATION_JOURNAL_SCHEMA_ID: &str = "cargo-allow.publication-journal.v1";
pub const PUBLICATION_JOURNAL_SCHEMA_VERSION: u32 = 1;

/// The exact selected operation this journal may carry.
pub const PUBLICATION_JOURNAL_OPERATION: &str = "publish_cargo_allow_final_0_2_0";
/// Recovery journals bind the original journal instead of continuing it.
pub const PUBLICATION_JOURNAL_RECOVERY_OPERATION: &str = "recover_cargo_allow_final_publication";
/// Digest chaining starts here; the first entry must reference it.
pub const PUBLICATION_JOURNAL_GENESIS_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
/// Bounded push retries after an observed-absent reconciliation.
pub const PUBLICATION_JOURNAL_MAX_UPLOAD_ATTEMPTS: u32 = 3;
/// Bounded provider note: leak prevention by construction, not screening.
pub const PUBLICATION_JOURNAL_MAX_RESPONSE_DETAIL_LEN: usize = 256;

/// Journal operation class, mirroring the production #3940 vocabulary.
/// Clean journals never become recovery journals; recovery starts a new
/// journal bound to the original via `prior_journal_digest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationJournalClassV1 {
    CleanFinalPublication,
    IncidentRecovery,
}

/// Ordered journal event vocabulary (#3921).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationJournalEventV1 {
    OperationSelected,
    AuthorizationConsumed,
    TagObservedExact,
    RowPreflightComplete,
    UploadIntentDurable,
    UploadRequestStarted,
    UploadResponseObserved,
    UploadResponseUnknown,
    RegistryObservationStarted,
    RegistryVisibleExact,
    RegistryVisibleConflict,
    RegistryObservedAbsent,
    RegistryWaiting,
    OperationIncident,
    OperationComplete,
}

/// Provider response class. There is no failure verdict: a missing or
/// unsuccessful response is `UploadResponseUnknown` with a class naming the
/// last known position, never an inference about the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadResponseClassV1 {
    Success,
    TransportError,
    NonzeroExit,
    Timeout,
    Unknown,
}

/// One package row the journal tracks. `candidate_archive_digest` is the
/// exact custody-bound archive digest #2502 supplies; the journal records it
/// verbatim and never substitutes moving-`main` bytes for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationJournalRowV1 {
    pub package_name: String,
    pub version: String,
    pub row_order: u32,
    pub candidate_archive_digest: String,
    pub depends_on: Vec<String>,
}

/// Caller-supplied registry observation. Reachability, visibility, and
/// digest agreement are separate claims: an unreachable provider is never
/// inferred as absent, and a visible row with a mismatched digest is a
/// conflict, never exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationRegistryObservationV1 {
    pub provider_reachable: bool,
    pub row_visible: bool,
    pub archive_digest_matches: bool,
}

/// Caller-supplied upload response with a bounded note. The note carries
/// operational position only; unbounded provider bodies and credentials have
/// no field to live in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationUploadResponseV1 {
    pub class: UploadResponseClassV1,
    pub detail: String,
}

/// One append-only journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowPublicationJournalEntryV1 {
    pub sequence: u64,
    pub previous_digest: String,
    pub entry_digest: String,
    pub kind: PublicationJournalEventV1,
    pub package_name: Option<String>,
    pub row_order: Option<u32>,
    pub candidate_archive_digest: Option<String>,
    pub at_unix_seconds: u64,
    pub reason: String,
    pub response_class: Option<UploadResponseClassV1>,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
}

/// The publication journal record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowPublicationJournalV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub journal_id: String,
    pub operation_id: String,
    pub operation_class: PublicationJournalClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    /// Recovery journals name the original clean journal; clean journals
    /// carry `None` and can never gain one later.
    pub prior_journal_digest: Option<String>,
    pub created_at_unix_seconds: u64,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
    /// The exact bound row set. Declared once, never extended.
    pub rows: Vec<PublicationJournalRowV1>,
    pub entries: Vec<CargoAllowPublicationJournalEntryV1>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// Caller-supplied construction inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationJournalInitV1 {
    pub journal_id: String,
    pub operation_id: String,
    pub operation_class: PublicationJournalClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub prior_journal_digest: Option<String>,
    /// The exact bound row set, usually from the frozen candidate. Rows
    /// cannot be added later: completion requires every declared row
    /// terminally resolved, so an omitted row can never vanish silently.
    pub rows: Vec<PublicationJournalRowV1>,
    pub created_at_unix_seconds: u64,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
}

/// Caller-supplied append request. Sequence and chaining are assigned by the
/// journal, never the caller, so a sequence number or previous digest cannot
/// be reused or forged through this API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationJournalAppendV1 {
    pub kind: PublicationJournalEventV1,
    pub row: Option<PublicationJournalRowV1>,
    pub response: Option<PublicationUploadResponseV1>,
    pub observation: Option<PublicationRegistryObservationV1>,
    pub at_unix_seconds: u64,
    pub reason: String,
}

const CLAIM_BOUNDARY: &str = "This record owns one append-only, crash-consistent history per cargo-allow package publication attempt: durable pre-intent, observed or unknown responses, provider observation verdicts, and incident preservation. It does not upload packages, observe the registry, authorize the operation, or execute recovery.";

/// Canonical lowercase hexadecimal: uppercase forms identify the same
/// object but chain to different entry digests, so they are rejected rather
/// than normalized. Converged with the #3930/#3927/#3925 contracts.
fn lower_hex_shape(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn digest_shape(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && lower_hex_shape(hex))
}

fn entry_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

/// Canonical digest input: every chained field, nothing ambient.
#[derive(Serialize)]
struct JournalEntryDigestInputV1<'a> {
    sequence: u64,
    previous_digest: &'a str,
    kind: PublicationJournalEventV1,
    package_name: Option<&'a str>,
    row_order: Option<u32>,
    candidate_archive_digest: Option<&'a str>,
    at_unix_seconds: u64,
    reason: &'a str,
    response_class: Option<UploadResponseClassV1>,
    workflow: &'a str,
    run: &'a str,
    attempt: &'a str,
    job: &'a str,
}

fn validate_row(row: &PublicationJournalRowV1) -> Result<(), &'static str> {
    if row.package_name.trim().is_empty() || row.version.trim().is_empty() {
        return Err("journal rows require a package name and version");
    }
    if !digest_shape(&row.candidate_archive_digest) {
        return Err("journal rows require a canonical candidate archive digest");
    }
    if row.depends_on.iter().any(|name| name.trim().is_empty()) {
        return Err("journal row dependencies must name packages");
    }
    if row.depends_on.iter().any(|name| name == &row.package_name) {
        return Err("journal rows cannot depend on themselves");
    }
    Ok(())
}

/// Begin a journal bound to one operation. Recovery journals must name the
/// original clean journal; clean journals carry no prior digest.
pub fn begin_publication_journal_v1(
    init: PublicationJournalInitV1,
) -> Result<CargoAllowPublicationJournalV1, &'static str> {
    if init.journal_id.trim().is_empty() || init.operation_id.trim().is_empty() {
        return Err("journal requires operation and journal identity");
    }
    for value in [
        init.authorization_digest.as_str(),
        init.custody_digest.as_str(),
        init.freeze_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("journal requires canonical operation identity digests");
        }
    }
    if init.operation_class == PublicationJournalClassV1::IncidentRecovery
        && init.prior_journal_digest.is_none()
    {
        return Err("recovery journals require the original journal digest");
    }
    if let Some(prior) = init.prior_journal_digest.as_deref() {
        if init.operation_class != PublicationJournalClassV1::IncidentRecovery {
            return Err("only a recovery journal may bind a prior journal");
        }
        if !digest_shape(prior) {
            return Err("recovery journals require a canonical prior journal digest");
        }
    }
    if init.created_at_unix_seconds < 1 {
        return Err("journal requires a positive construction time");
    }
    if init.rows.is_empty() {
        return Err("journal requires a non-empty bound row set");
    }
    for row in &init.rows {
        validate_row(row)?;
    }
    let mut seen: Vec<&str> = Vec::new();
    for row in &init.rows {
        if seen.contains(&row.package_name.as_str()) {
            return Err("journal row names must be unique");
        }
        seen.push(row.package_name.as_str());
    }
    for value in [
        init.workflow.as_str(),
        init.run.as_str(),
        init.attempt.as_str(),
        init.job.as_str(),
    ] {
        if value.trim().is_empty() {
            return Err("journal requires workflow/run/attempt/job identity");
        }
    }
    Ok(CargoAllowPublicationJournalV1 {
        schema_id: PUBLICATION_JOURNAL_SCHEMA_ID.to_string(),
        schema_version: PUBLICATION_JOURNAL_SCHEMA_VERSION,
        journal_id: init.journal_id,
        operation_id: init.operation_id,
        operation_class: init.operation_class,
        authorization_digest: init.authorization_digest,
        custody_digest: init.custody_digest,
        freeze_digest: init.freeze_digest,
        prior_journal_digest: init.prior_journal_digest,
        created_at_unix_seconds: init.created_at_unix_seconds,
        workflow: init.workflow,
        run: init.run,
        attempt: init.attempt,
        job: init.job,
        rows: init.rows,
        entries: Vec::new(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_upload_packages".to_string(),
            "does_not_observe_registry".to_string(),
            "does_not_authorize_operation".to_string(),
        ],
    })
}

fn journal_has(journal: &CargoAllowPublicationJournalV1, kind: PublicationJournalEventV1) -> bool {
    journal.entries.iter().any(|entry| entry.kind == kind)
}

/// Declared-row fidelity: every row-bearing append must repeat the exact
/// bound row, so digests, order, and dependencies cannot be swapped
/// mid-operation.
fn declared_row<'a>(
    journal: &'a CargoAllowPublicationJournalV1,
    row: &'a PublicationJournalRowV1,
) -> Result<&'a PublicationJournalRowV1, &'static str> {
    journal
        .rows
        .iter()
        .find(|declared| declared.package_name == row.package_name)
        .filter(|declared| *declared == row)
        .ok_or("journal rows must match the bound row set exactly")
}

fn row_has(
    journal: &CargoAllowPublicationJournalV1,
    package: &str,
    kind: PublicationJournalEventV1,
) -> bool {
    journal
        .entries
        .iter()
        .any(|entry| entry.kind == kind && entry.package_name.as_deref() == Some(package))
}

fn row_upload_attempts(journal: &CargoAllowPublicationJournalV1, package: &str) -> u32 {
    journal
        .entries
        .iter()
        .filter(|entry| {
            entry.kind == PublicationJournalEventV1::UploadRequestStarted
                && entry.package_name.as_deref() == Some(package)
        })
        .count() as u32
}

fn row_latest_kind(
    journal: &CargoAllowPublicationJournalV1,
    package: &str,
) -> Option<PublicationJournalEventV1> {
    journal
        .entries
        .iter()
        .rev()
        .find(|entry| entry.package_name.as_deref() == Some(package))
        .map(|entry| entry.kind)
}

fn operation_incident_recorded(journal: &CargoAllowPublicationJournalV1) -> bool {
    journal_has(journal, PublicationJournalEventV1::OperationIncident)
}

/// Row terminality: exact visibility ends a row; conflict ends it only with
/// a recorded operation incident; unknown and waiting never terminate.
fn row_terminally_resolved(journal: &CargoAllowPublicationJournalV1, package: &str) -> bool {
    match row_latest_kind(journal, package) {
        Some(PublicationJournalEventV1::RegistryVisibleExact) => true,
        Some(PublicationJournalEventV1::RegistryVisibleConflict) => {
            operation_incident_recorded(journal)
        }
        _ => false,
    }
}

fn check_row_preflight(
    journal: &CargoAllowPublicationJournalV1,
    row: &PublicationJournalRowV1,
) -> Result<(), &'static str> {
    use PublicationJournalEventV1 as Event;
    if !journal_has(journal, Event::TagObservedExact) {
        return Err("row preflight requires the exact observed tag");
    }
    if row_has(journal, &row.package_name, Event::RowPreflightComplete) {
        return Err("row preflight is recorded exactly once per row");
    }
    for dependency in &row.depends_on {
        if !row_has(journal, dependency, Event::RegistryVisibleExact) {
            return Err("dependent rows wait for exact dependency visibility");
        }
    }
    Ok(())
}

/// Append one event. Ordering, chaining, and legality are enforced here; a
/// refused append is a stop signal for the caller, never a skipped log line.
pub fn append_journal_event_v1(
    journal: &mut CargoAllowPublicationJournalV1,
    append: PublicationJournalAppendV1,
) -> Result<(), &'static str> {
    use PublicationJournalEventV1 as Event;
    if append.reason.trim().is_empty() {
        return Err("journal entries require a reason");
    }
    if secret_marker(&append.reason).is_some() {
        return Err("secret material must never enter journal records");
    }
    if append
        .response
        .as_ref()
        .is_some_and(|response| response.detail.len() > PUBLICATION_JOURNAL_MAX_RESPONSE_DETAIL_LEN)
    {
        return Err("journal response notes are bounded and carry no bodies");
    }
    let baseline = journal
        .entries
        .last()
        .map(|previous| previous.at_unix_seconds)
        .unwrap_or(journal.created_at_unix_seconds);
    if append.at_unix_seconds < baseline {
        return Err("journal entries must not predate construction or predecessors");
    }
    if let Some(row) = append.row.as_ref() {
        validate_row(row)?;
        declared_row(journal, row)?;
    }
    // Legality per kind. Row-scoped kinds require a row; operation-scoped
    // kinds refuse one so operation history cannot hide inside row history.
    let row_name: Option<&str> = match append.kind {
        Event::OperationSelected
        | Event::AuthorizationConsumed
        | Event::TagObservedExact
        | Event::OperationIncident
        | Event::OperationComplete => {
            if append.row.is_some() {
                return Err("operation events carry no row");
            }
            None
        }
        Event::RowPreflightComplete
        | Event::UploadIntentDurable
        | Event::UploadRequestStarted
        | Event::UploadResponseObserved
        | Event::UploadResponseUnknown
        | Event::RegistryObservationStarted
        | Event::RegistryVisibleExact
        | Event::RegistryVisibleConflict
        | Event::RegistryObservedAbsent
        | Event::RegistryWaiting => Some(
            append
                .row
                .as_ref()
                .map(|row| row.package_name.as_str())
                .ok_or("row events require a row")?,
        ),
    };
    match append.kind {
        Event::OperationSelected => {
            if journal_has(journal, Event::OperationSelected) {
                return Err("one journal binds exactly one operation selection");
            }
        }
        Event::AuthorizationConsumed => {
            if !journal_has(journal, Event::OperationSelected) {
                return Err("authorization consumption requires a selected operation");
            }
            if journal_has(journal, Event::AuthorizationConsumed) {
                return Err("authorization is consumed exactly once per journal");
            }
        }
        Event::TagObservedExact => {
            if !journal_has(journal, Event::AuthorizationConsumed) {
                return Err("exact tag observation requires consumed authorization");
            }
            if journal_has(journal, Event::TagObservedExact) {
                return Err("the exact tag is observed exactly once per journal");
            }
        }
        Event::RowPreflightComplete => {
            check_row_preflight(
                journal,
                append.row.as_ref().ok_or("row events require a row")?,
            )?;
        }
        Event::UploadIntentDurable => {
            let row = append.row.as_ref().ok_or("row events require a row")?;
            if operation_incident_recorded(journal) {
                return Err(
                    "clean authorization cannot continue after an incident; recovery starts a new journal",
                );
            }
            if !row_has(journal, &row.package_name, Event::RowPreflightComplete) {
                return Err("upload intent requires row preflight");
            }
            if row_has(journal, &row.package_name, Event::UploadIntentDurable) {
                // Bounded retry: a fresh intent is allowed only after an
                // unreconciled unknown or a proven absence, within the
                // attempt bound. Observed, exact, conflict, and waiting rows
                // never re-invoke.
                match row_latest_kind(journal, &row.package_name) {
                    Some(Event::UploadResponseUnknown | Event::RegistryObservedAbsent) => {}
                    _ => {
                        return Err(
                            "upload intent is durable once per attempt; retry requires an unreconciled unknown or proven absence",
                        );
                    }
                }
                if row_upload_attempts(journal, &row.package_name)
                    >= PUBLICATION_JOURNAL_MAX_UPLOAD_ATTEMPTS
                {
                    return Err("upload attempt bound reached; operator decision required");
                }
            }
        }
        Event::UploadRequestStarted => {
            let package = row_name.ok_or("row events require a row")?;
            if row_latest_kind(journal, package) != Some(Event::UploadIntentDurable) {
                return Err("upload requests start only from durable intent");
            }
        }
        Event::UploadResponseObserved | Event::UploadResponseUnknown => {
            let package = row_name.ok_or("row events require a row")?;
            if row_latest_kind(journal, package) != Some(Event::UploadRequestStarted) {
                return Err("responses belong to a started upload only");
            }
            if append.response.is_none() {
                return Err("responses require a response class");
            }
        }
        Event::RegistryObservationStarted => {
            let package = row_name.ok_or("row events require a row")?;
            match row_latest_kind(journal, package) {
                Some(Event::UploadResponseObserved)
                | Some(Event::UploadResponseUnknown)
                | Some(Event::RegistryWaiting) => {}
                _ => return Err("registry observation follows an upload response"),
            }
        }
        Event::RegistryObservedAbsent => {
            let package = row_name.ok_or("row events require a row")?;
            let observation = append
                .observation
                .as_ref()
                .ok_or("absence verdicts require provider observation")?;
            if !observation.provider_reachable || observation.row_visible {
                return Err("absence requires a reachable provider and an invisible row");
            }
            match row_latest_kind(journal, package) {
                Some(Event::UploadResponseObserved)
                | Some(Event::UploadResponseUnknown)
                | Some(Event::RegistryObservationStarted)
                | Some(Event::RegistryWaiting) => {}
                _ => return Err("absence verdicts reconcile an upload response"),
            }
        }
        Event::RegistryVisibleExact => {
            let package = row_name.ok_or("row events require a row")?;
            let observation = append
                .observation
                .as_ref()
                .ok_or("exact visibility requires provider observation")?;
            if !observation.provider_reachable || !observation.row_visible {
                return Err("exact visibility requires a reachable, visible row");
            }
            if !observation.archive_digest_matches {
                return Err("a digest mismatch is a conflict, never exact");
            }
            match row_latest_kind(journal, package) {
                Some(Event::UploadResponseObserved)
                | Some(Event::UploadResponseUnknown)
                | Some(Event::RegistryObservationStarted)
                | Some(Event::RegistryWaiting) => {}
                _ => return Err("exact visibility reconciles an upload response"),
            }
        }
        Event::RegistryVisibleConflict => {
            let package = row_name.ok_or("row events require a row")?;
            if append.observation.is_none() {
                return Err("conflict verdicts require provider observation");
            }
            match row_latest_kind(journal, package) {
                Some(Event::UploadResponseObserved)
                | Some(Event::UploadResponseUnknown)
                | Some(Event::RegistryObservationStarted)
                | Some(Event::RegistryWaiting) => {}
                _ => return Err("conflict verdicts reconcile an upload response"),
            }
        }
        Event::RegistryWaiting => {
            let package = row_name.ok_or("row events require a row")?;
            match row_latest_kind(journal, package) {
                Some(Event::UploadResponseObserved)
                | Some(Event::UploadResponseUnknown)
                | Some(Event::RegistryObservationStarted) => {}
                _ => return Err("waiting follows an upload response or observation start"),
            }
        }
        Event::OperationIncident => {
            if !journal_has(journal, Event::OperationSelected) {
                return Err("incidents belong to a selected operation");
            }
        }
        Event::OperationComplete => {
            if !journal_has(journal, Event::TagObservedExact) {
                return Err("completion requires the exact observed tag");
            }
            // Every declared row must be terminally resolved: an omitted
            // row can never vanish silently from the completed aggregate.
            let mut unresolved: Option<String> = None;
            for row in &journal.rows {
                if !row_terminally_resolved(journal, &row.package_name) {
                    unresolved = Some(row.package_name.clone());
                    break;
                }
            }
            if let Some(package) = unresolved {
                return Err(match row_latest_kind(journal, &package) {
                    Some(Event::RegistryVisibleConflict) => {
                        "completion with a conflict requires a recorded operation incident"
                    }
                    _ => "completion requires every declared row terminally resolved",
                });
            }
        }
    }
    let sequence = journal.entries.len() as u64 + 1;
    let previous_digest = journal
        .entries
        .last()
        .map(|previous| previous.entry_digest.clone())
        .unwrap_or_else(|| PUBLICATION_JOURNAL_GENESIS_DIGEST.to_string());
    let row = append.row.as_ref();
    let input = JournalEntryDigestInputV1 {
        sequence,
        previous_digest: &previous_digest,
        kind: append.kind,
        package_name: row.map(|row| row.package_name.as_str()),
        row_order: row.map(|row| row.row_order),
        candidate_archive_digest: row.map(|row| row.candidate_archive_digest.as_str()),
        at_unix_seconds: append.at_unix_seconds,
        reason: &append.reason,
        response_class: append.response.as_ref().map(|response| response.class),
        workflow: &journal.workflow,
        run: &journal.run,
        attempt: &journal.attempt,
        job: &journal.job,
    };
    let entry_digest = entry_digest(&input).map_err(|_| "journal entry digest failed")?;
    journal.entries.push(CargoAllowPublicationJournalEntryV1 {
        sequence,
        previous_digest,
        entry_digest,
        kind: append.kind,
        package_name: row.map(|row| row.package_name.clone()),
        row_order: row.map(|row| row.row_order),
        candidate_archive_digest: row.map(|row| row.candidate_archive_digest.clone()),
        at_unix_seconds: append.at_unix_seconds,
        reason: append.reason,
        response_class: append.response.map(|response| response.class),
        workflow: journal.workflow.clone(),
        run: journal.run.clone(),
        attempt: journal.attempt.clone(),
        job: journal.job.clone(),
    });
    Ok(())
}

/// Recompute every link and digest. Detects edited entries, reordered
/// entries, and broken chains after retries or storage faults.
pub fn verify_publication_journal_v1(
    journal: &CargoAllowPublicationJournalV1,
) -> Result<(), &'static str> {
    let mut previous = PUBLICATION_JOURNAL_GENESIS_DIGEST.to_string();
    for (index, entry) in journal.entries.iter().enumerate() {
        if entry.sequence != index as u64 + 1 {
            return Err("journal sequence must be gapless from one");
        }
        if entry.previous_digest != previous {
            return Err("journal previous digest must chain to the prior entry");
        }
        let input = JournalEntryDigestInputV1 {
            sequence: entry.sequence,
            previous_digest: &entry.previous_digest,
            kind: entry.kind,
            package_name: entry.package_name.as_deref(),
            row_order: entry.row_order,
            candidate_archive_digest: entry.candidate_archive_digest.as_deref(),
            at_unix_seconds: entry.at_unix_seconds,
            reason: &entry.reason,
            response_class: entry.response_class,
            workflow: &entry.workflow,
            run: &entry.run,
            attempt: &entry.attempt,
            job: &entry.job,
        };
        let recomputed = entry_digest(&input).map_err(|_| "journal entry digest failed")?;
        if recomputed != entry.entry_digest {
            return Err("journal entry digest mismatch; entries are append-only");
        }
        previous = entry.entry_digest.clone();
    }
    Ok(())
}

/// Canonical JSON renderer for publication journals.
pub fn render_publication_journal_v1(
    journal: &CargoAllowPublicationJournalV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(journal)
}
