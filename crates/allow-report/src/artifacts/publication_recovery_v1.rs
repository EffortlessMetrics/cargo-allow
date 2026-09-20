//! Deterministic unknown-upload recovery for the final release (#3924).
//!
//! The #3921 journal records an `UploadResponseUnknown` when the runner
//! cannot know whether the provider accepted the request, and the #3922
//! checkpoints preserve that history across total runner loss. This module
//! decides the only safe next transition for one package row from exact
//! evidence: the live journal, verified checkpoint references, original
//! custody, authorization identities, and bounded multi-surface provider
//! observations.
//!
//! `UploadResponseUnknown` is neither failure nor success. Exact visibility
//! means skip, never re-upload; conflict means incident and stop; absence
//! after bounded observation may permit the original custody bytes to be
//! uploaded exactly once, and only under exact recovery authorization. A
//! provider outage is never absence, and recovery never rebuilds the row
//! from moving source: it continues from the original custody bytes under
//! append-only history that no decision in this module can rewrite.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! uploads, no credential reads, no registry observation fetching, and no
//! live-state mutation. Observations are caller-supplied multi-surface
//! claims; the decision classifies them but never fetches them. Checkpoint
//! references are caller-verified through the #3922 API before deciding:
//! this model consumes verified positions, it does not re-verify them.

use serde::{Deserialize, Serialize};

use super::publication_checkpoint_v1::PublicationCheckpointKindV1;
use super::publication_journal_v1::{
    CargoAllowPublicationJournalV1, PUBLICATION_JOURNAL_GENESIS_DIGEST, PublicationJournalClassV1,
    PublicationJournalEventV1, PublicationJournalRowV1,
};
use super::release_operation_authority_v1::{
    CargoAllowReleaseOperationIdentityV1, release_operation_identity_digest_v1,
    validate_release_operation_identity_v1,
};

pub const PUBLICATION_RECOVERY_SCHEMA_ID: &str = "cargo-allow.publication-recovery.v1";
pub const PUBLICATION_RECOVERY_SCHEMA_VERSION: u32 = 1;
/// Bounded read-only observation rounds before absence may be considered.
/// Absence is never inferred from fewer rounds, and never from a silent
/// surface no matter how many rounds elapsed.
pub const PUBLICATION_RECOVERY_MAX_OBSERVATION_ROUNDS: u32 = 5;

/// Recovery dispositions. There is no failure verdict: an unknown outcome
/// stays unknown until read-only observation resolves it, and every
/// upload path stays closed until its exact preconditions hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownUploadRecoveryClassV1 {
    NotAttemptedSafeToStart,
    ResponseUnknownObservationRequired,
    VisibleExactSkipAndContinue,
    WaitingForPropagation,
    VisibleConflictIncident,
    MissingAfterBoundedObservationRecoveryMayUpload,
    ProviderUnavailableStop,
    RecoveryAuthorizationRequired,
    InstrumentFailure,
}

/// One provider surface claim. Reachability, visibility, and digest
/// agreement are separate: an unreachable surface says nothing, an
/// invisible surface is not absence by itself, and a digest verdict binds
/// visible rows only. `digest_matches` is `None` when the surface cannot
/// compare archive bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySurfaceObservationV1 {
    pub reachable: bool,
    pub visible: bool,
    pub digest_matches: Option<bool>,
}

/// Bounded multi-surface registry observation: the provider API, the
/// registry index, a download probe, and the resolver view, plus how many
/// read-only rounds produced them and whether the observation harness
/// itself failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRegistryObservationsV1 {
    pub api: RecoverySurfaceObservationV1,
    pub index: RecoverySurfaceObservationV1,
    pub download: RecoverySurfaceObservationV1,
    pub resolver: RecoverySurfaceObservationV1,
    pub rounds: u32,
    pub instrument_failure: bool,
}

/// Original custody: the exact candidate bytes the freeze bound, plus the
/// custody and freeze digests that authorize them. Recovery continues from
/// these bytes; a row naming any other candidate digest is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCustodyV1 {
    pub candidate_archive_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
}

/// Exact recovery authorization: its own digest, the recovery plan it
/// executes, and the original candidate it is bound to. Clean
/// authorization never substitutes for it once an incident exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAuthorizationV1 {
    pub authorization_digest: String,
    pub plan_digest: String,
    pub bound_candidate_digest: String,
}

/// Authorization identities available to the recovery lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAuthorizationsV1 {
    pub clean_authorization_digest: String,
    pub recovery: Option<RecoveryAuthorizationV1>,
}

/// A caller-verified checkpoint position. Verification happens through the
/// #3922 API (exact identity, prefix integrity, producer trust, readback
/// witness) before deciding; this model consumes verified positions and
/// never re-verifies them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCheckpointPositionV1 {
    pub operation_id: String,
    /// Canonical #3940 operation identity digest. Positions never invent
    /// operation identity: this must equal the bound journal's digest.
    pub operation_identity_digest: String,
    pub checkpoint_sequence: u64,
    pub journal_head_sequence: u64,
    pub journal_head_digest: String,
    pub kind: PublicationCheckpointKindV1,
    pub readback_complete: bool,
}

/// One deterministic recovery disposition for one package row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowUnknownUploadRecoveryV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_id: String,
    /// Canonical #3940 operation identity digest the decision was taken
    /// under, copied from the bound journal.
    pub operation_identity_digest: String,
    pub operation_class: PublicationJournalClassV1,
    pub package_name: String,
    pub row_order: u32,
    pub class: UnknownUploadRecoveryClassV1,
    /// The journal head this decision was taken against. Later history never
    /// rewrites it; a new decision supersedes by reference, not by edit.
    pub decided_journal_head_sequence: u64,
    pub decided_journal_head_digest: String,
    pub rounds_consumed: u32,
    /// True when the disposition consumes recovery authority (MayUpload) or
    /// waits for it (RecoveryAuthorizationRequired).
    pub requires_recovery_authorization: bool,
    /// The journal incident posture at decision time, preserved verbatim so
    /// a later success can never erase it.
    pub preserved_incident: bool,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

const CLAIM_BOUNDARY: &str = "This record owns one deterministic recovery disposition per package row with an unknown upload outcome: exact operation and row identity, the decided journal head, consumed observation rounds, authorization posture, and preserved incident lineage. It does not upload packages, observe the registry, authorize recovery, or rewrite history.";

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

/// The bound row must repeat the journal denominator exactly, so a stale or
/// foreign candidate can never enter recovery.
fn declared_row<'a>(
    journal: &'a CargoAllowPublicationJournalV1,
    row: &PublicationJournalRowV1,
) -> Result<&'a PublicationJournalRowV1, &'static str> {
    journal
        .rows
        .iter()
        .find(|declared| declared.package_name == row.package_name)
        .filter(|declared| *declared == row)
        .ok_or("recovery rows must match the bound journal row set exactly")
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

fn row_request_started(journal: &CargoAllowPublicationJournalV1, package: &str) -> bool {
    journal.entries.iter().any(|entry| {
        entry.kind == PublicationJournalEventV1::UploadRequestStarted
            && entry.package_name.as_deref() == Some(package)
    })
}

fn journal_has_incident(journal: &CargoAllowPublicationJournalV1) -> bool {
    journal
        .entries
        .iter()
        .any(|entry| entry.kind == PublicationJournalEventV1::OperationIncident)
}

fn journal_head(journal: &CargoAllowPublicationJournalV1) -> (u64, String) {
    journal
        .entries
        .last()
        .map(|entry| (entry.sequence, entry.entry_digest.clone()))
        .unwrap_or_else(|| (0, PUBLICATION_JOURNAL_GENESIS_DIGEST.to_string()))
}

/// A verified pre-intent position for this row's upload: the referenced
/// journal entry must still be present, must be the row's durable upload
/// intent, and must repeat the row's identity exactly, so the intent proof
/// cannot drift to another row or a rewritten prefix.
fn verified_pre_intent_position<'a>(
    journal: &CargoAllowPublicationJournalV1,
    row: &PublicationJournalRowV1,
    checkpoints: &'a [RecoveryCheckpointPositionV1],
) -> Option<&'a RecoveryCheckpointPositionV1> {
    checkpoints.iter().find(|position| {
        position.kind == PublicationCheckpointKindV1::PreIntentDurable
            && position.readback_complete
            && position.operation_id == journal.operation_id
            && position.operation_identity_digest == journal.operation_identity_digest
            && journal.entries.iter().any(|entry| {
                entry.sequence == position.journal_head_sequence
                    && entry.entry_digest == position.journal_head_digest
                    && entry.kind == PublicationJournalEventV1::UploadIntentDurable
                    && entry.package_name.as_deref() == Some(row.package_name.as_str())
                    && entry.row_order == Some(row.row_order)
                    && entry.candidate_archive_digest.as_deref()
                        == Some(row.candidate_archive_digest.as_str())
            })
    })
}

fn finish(
    journal: &CargoAllowPublicationJournalV1,
    row: &PublicationJournalRowV1,
    class: UnknownUploadRecoveryClassV1,
    rounds_consumed: u32,
    requires_recovery_authorization: bool,
) -> CargoAllowUnknownUploadRecoveryV1 {
    let (sequence, digest) = journal_head(journal);
    CargoAllowUnknownUploadRecoveryV1 {
        schema_id: PUBLICATION_RECOVERY_SCHEMA_ID.to_string(),
        schema_version: PUBLICATION_RECOVERY_SCHEMA_VERSION,
        operation_id: journal.operation_id.clone(),
        operation_identity_digest: journal.operation_identity_digest.clone(),
        operation_class: journal.operation_class,
        package_name: row.package_name.clone(),
        row_order: row.row_order,
        class,
        decided_journal_head_sequence: sequence,
        decided_journal_head_digest: digest,
        rounds_consumed,
        requires_recovery_authorization,
        preserved_incident: journal_has_incident(journal),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_upload_packages".to_string(),
            "does_not_observe_registry".to_string(),
            "does_not_authorize_operation".to_string(),
        ],
    }
}

/// Decide the only safe next transition for one row with an unknown upload
/// outcome. Unknown stays unknown: every upload path below stays closed
/// until exact preconditions hold, and no path ever infers success,
/// failure, or absence from silence.
pub fn decide_unknown_upload_recovery_v1(
    journal: &CargoAllowPublicationJournalV1,
    checkpoints: &[RecoveryCheckpointPositionV1],
    row: &PublicationJournalRowV1,
    custody: &RecoveryCustodyV1,
    authorizations: &RecoveryAuthorizationsV1,
    observations: &RecoveryRegistryObservationsV1,
) -> Result<CargoAllowUnknownUploadRecoveryV1, &'static str> {
    use PublicationJournalEventV1 as Event;
    use UnknownUploadRecoveryClassV1 as Class;
    declared_row(journal, row)?;
    if row.candidate_archive_digest != custody.candidate_archive_digest {
        return Err("recovery continues from the original custody bytes only");
    }
    for value in [
        custody.candidate_archive_digest.as_str(),
        custody.custody_digest.as_str(),
        custody.freeze_digest.as_str(),
        authorizations.clean_authorization_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("recovery requires canonical custody and authorization identity");
        }
    }
    if let Some(recovery) = authorizations.recovery.as_ref() {
        for value in [
            recovery.authorization_digest.as_str(),
            recovery.plan_digest.as_str(),
            recovery.bound_candidate_digest.as_str(),
        ] {
            if !digest_shape(value) {
                return Err("recovery authorization must carry canonical identity");
            }
        }
    }
    if custody.custody_digest != journal.custody_digest
        || custody.freeze_digest != journal.freeze_digest
        || authorizations.clean_authorization_digest != journal.authorization_digest
    {
        return Err("recovery custody and authorization must agree with the journal header");
    }
    for surface in [
        observations.api,
        observations.index,
        observations.download,
        observations.resolver,
    ] {
        if (surface.visible && !surface.reachable)
            || (surface.digest_matches.is_some() && !surface.visible)
        {
            return Err("recovery surfaces must be internally consistent");
        }
    }
    for position in checkpoints {
        if position.operation_id.trim().is_empty() {
            return Err("checkpoint positions must name their operation");
        }
        if !digest_shape(&position.journal_head_digest)
            || !digest_shape(&position.operation_identity_digest)
        {
            return Err("checkpoint positions must bind a canonical journal head");
        }
        if position.operation_identity_digest != journal.operation_identity_digest {
            return Err("checkpoint positions must belong to the decided operation");
        }
    }
    if observations.instrument_failure {
        return Ok(finish(
            journal,
            row,
            Class::InstrumentFailure,
            observations.rounds,
            false,
        ));
    }
    // History stands: an exactly reconciled row is skipped, never re-uploaded.
    if row_latest_kind(journal, &row.package_name) == Some(Event::RegistryVisibleExact) {
        return Ok(finish(
            journal,
            row,
            Class::VisibleExactSkipAndContinue,
            observations.rounds,
            false,
        ));
    }
    let surfaces = [
        observations.api,
        observations.index,
        observations.download,
        observations.resolver,
    ];
    // Conflict first and fail-closed: any visible row with a mismatched
    // digest is an incident, even when another surface agrees.
    let conflict = surfaces
        .iter()
        .any(|surface| surface.visible && surface.digest_matches == Some(false));
    if conflict {
        return Ok(finish(
            journal,
            row,
            Class::VisibleConflictIncident,
            observations.rounds,
            false,
        ));
    }
    let reachable = surfaces.iter().filter(|surface| surface.reachable).count();
    if reachable == 0 {
        return Ok(finish(
            journal,
            row,
            Class::ProviderUnavailableStop,
            observations.rounds,
            false,
        ));
    }
    // Checksum equality on any reachable surface is exact visibility: skip.
    let exact = surfaces
        .iter()
        .any(|surface| surface.visible && surface.digest_matches == Some(true));
    if exact {
        return Ok(finish(
            journal,
            row,
            Class::VisibleExactSkipAndContinue,
            observations.rounds,
            false,
        ));
    }
    // A visible row the surfaces cannot compare yet is propagation in
    // flight, never absence and never permission.
    let visible_unknown = surfaces
        .iter()
        .any(|surface| surface.visible && surface.digest_matches.is_none());
    if visible_unknown {
        return Ok(finish(
            journal,
            row,
            Class::WaitingForPropagation,
            observations.rounds,
            false,
        ));
    }
    // Nothing was ever sent, so there is no outcome to observe: a fresh
    // start is safe under clean authorization and a clean journal. An
    // incident anywhere still demands recovery authority first.
    if !row_request_started(journal, &row.package_name) {
        if journal_has_incident(journal) {
            return Ok(finish(
                journal,
                row,
                Class::RecoveryAuthorizationRequired,
                observations.rounds,
                true,
            ));
        }
        return Ok(finish(
            journal,
            row,
            Class::NotAttemptedSafeToStart,
            observations.rounds,
            false,
        ));
    }
    // Nothing is visible anywhere. Silence is not absence until every
    // surface answered reachably through the full bounded round budget.
    let all_answered_absent = surfaces
        .iter()
        .all(|surface| surface.reachable && !surface.visible);
    if !all_answered_absent {
        if observations.rounds < PUBLICATION_RECOVERY_MAX_OBSERVATION_ROUNDS {
            return Ok(finish(
                journal,
                row,
                Class::ResponseUnknownObservationRequired,
                observations.rounds,
                false,
            ));
        }
        return Ok(finish(
            journal,
            row,
            Class::ProviderUnavailableStop,
            observations.rounds,
            false,
        ));
    }
    if observations.rounds < PUBLICATION_RECOVERY_MAX_OBSERVATION_ROUNDS {
        return Ok(finish(
            journal,
            row,
            Class::ResponseUnknownObservationRequired,
            observations.rounds,
            false,
        ));
    }
    // The bounded budget is spent and every surface answered absent. The
    // row was attempted (never-attempted rows returned above), so an
    // upload may have reached the provider unseen. Re-uploading the
    // original bytes requires exact recovery authority bound to the
    // original candidate first, and verified durable intent second: clean
    // authorization never substitutes here.
    let authorized = authorizations.recovery.as_ref().is_some_and(|recovery| {
        recovery.bound_candidate_digest == custody.candidate_archive_digest
    });
    if !authorized {
        return Ok(finish(
            journal,
            row,
            Class::RecoveryAuthorizationRequired,
            observations.rounds,
            true,
        ));
    }
    if verified_pre_intent_position(journal, row, checkpoints).is_none() {
        return Err("recovery uploads require a verified pre-intent checkpoint position");
    }
    Ok(finish(
        journal,
        row,
        Class::MissingAfterBoundedObservationRecoveryMayUpload,
        observations.rounds,
        true,
    ))
}

/// Decide the only safe next transition bound to one canonical #3940
/// release operation. The journal must carry the canonical identity digest;
/// positions for another operation never satisfy the decision.
pub fn decide_unknown_upload_recovery_for_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    journal: &CargoAllowPublicationJournalV1,
    checkpoints: &[RecoveryCheckpointPositionV1],
    row: &PublicationJournalRowV1,
    custody: &RecoveryCustodyV1,
    authorizations: &RecoveryAuthorizationsV1,
    observations: &RecoveryRegistryObservationsV1,
) -> Result<CargoAllowUnknownUploadRecoveryV1, &'static str> {
    validate_release_operation_identity_v1(identity)
        .map_err(|_| "recovery operation identity is not canonical")?;
    let identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    if journal.operation_identity_digest != identity_digest {
        return Err("recovery decides under the canonical operation only");
    }
    let disposition = decide_unknown_upload_recovery_v1(
        journal,
        checkpoints,
        row,
        custody,
        authorizations,
        observations,
    )?;
    if disposition.operation_identity_digest != identity_digest {
        return Err("recovery disposition must name the canonical operation");
    }
    Ok(disposition)
}

/// Dependants remain blocked until every prerequisite row is exactly
/// reconciled in the journal. A recovered row unlocks its dependants only
/// through `RegistryVisibleExact`, never through a recovery disposition
/// alone.
pub fn recovery_dependant_may_begin_v1(
    journal: &CargoAllowPublicationJournalV1,
    row: &PublicationJournalRowV1,
) -> Result<(), &'static str> {
    declared_row(journal, row)?;
    for dependency in &row.depends_on {
        if row_latest_kind(journal, dependency)
            != Some(PublicationJournalEventV1::RegistryVisibleExact)
        {
            return Err("dependants remain blocked until prerequisites are registry-visible exact");
        }
    }
    Ok(())
}

/// Canonical JSON renderer for recovery dispositions.
pub fn render_unknown_upload_recovery_v1(
    disposition: &CargoAllowUnknownUploadRecoveryV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(disposition)
}
