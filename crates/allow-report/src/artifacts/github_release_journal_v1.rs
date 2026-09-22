//! Append-only GitHub Release transaction journal for the final release (#3933).
//!
//! The #3726 provenance and actual-set authorities own asset semantics; this
//! module owns crash-consistent mutation history for one exact final-release
//! operation and GitHub Release identity: draft creation, exact asset
//! uploads, actual-state reconciliation, and one-way public finalization.
//! A later run must never create a second draft, duplicate or replace an
//! asset, publish an incomplete release, or erase the first uncertain or
//! incident state.
//!
//! Journals never invent operation identity: every record carries the
//! canonical #3940 `operation_identity_digest` derived from a revalidated
//! canonical identity. Unknown API responses are reconciled by exact
//! observation before any retry or continuation; same-name wrong-byte
//! assets are incidents, never replaced under a clean operation.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! release creation, mutation, or deletion, no credential reads, and no
//! live-state mutation. Observations are caller-supplied; the journal
//! classifies and chains them but never fetches them.

use serde::{Deserialize, Serialize};

use super::release_operation_authority_v1::{
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationIdentityV1,
    release_operation_identity_digest_v1, validate_release_operation_identity_v1,
};

pub const GITHUB_RELEASE_JOURNAL_SCHEMA_ID: &str = "cargo-allow.github-release-journal.v1";
pub const GITHUB_RELEASE_JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const GITHUB_RELEASE_JOURNAL_GENESIS_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Operation classes that may own a GitHub Release journal. Containment
/// never publishes: it has no draft, asset, or finalization path here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubReleaseJournalClassV1 {
    CleanFinalPublication,
    IncidentRecovery,
}

/// Append-only mutation/observation vocabulary. Correction is another event;
/// a later successful run can never omit an earlier unknown response,
/// conflict, replacement attempt, or premature-public incident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubReleaseJournalEventV1 {
    DraftCreateIntentDurable,
    DraftCreateStarted,
    DraftCreateResponseObserved,
    DraftCreateResponseUnknown,
    DraftObservedExact,
    AssetUploadIntentDurable,
    AssetUploadStarted,
    AssetUploadResponseObserved,
    AssetUploadResponseUnknown,
    AssetObservedExact,
    ExtraAssetObserved,
    ActualAssetSetReconciled,
    CloseoutReceiptComplete,
    FinalizeIntentDurable,
    FinalizeStarted,
    FinalizeResponseObserved,
    FinalizeResponseUnknown,
    PublicReleaseObservedExact,
    ReleaseIncident,
    OperationComplete,
}

/// One expected asset: role/name/size/SHA-256 plus producer and attestation
/// identity, derived only from the typed manifest/support/closeout
/// authorities. Same-name wrong-byte actuals are incidents, never replacements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubReleaseExpectedAssetV1 {
    pub asset_role: String,
    pub asset_name: String,
    pub size_bytes: u64,
    pub sha256_digest: String,
    pub producer_identity: String,
    pub attestation_identity: String,
}

/// One actual observed asset: ID/name/size/observed SHA-256 as returned by
/// the provider query/download path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubReleaseActualAssetV1 {
    pub asset_id: String,
    pub asset_name: String,
    pub size_bytes: u64,
    pub observed_sha256_digest: String,
}

/// The GitHub Release journal record: one append-only operation binding the
/// exact repository/tag identity, the known GitHub Release ID once observed,
/// draft/public/prerelease state, and the expected asset set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowGitHubReleaseJournalV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub journal_id: String,
    pub operation_id: String,
    /// Canonical #3940 operation identity digest. Journals never invent
    /// operation identity: this must equal the digest of the canonical
    /// release-operation identity the journal is bound to.
    pub operation_identity_digest: String,
    pub operation_class: GitHubReleaseJournalClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub repository: String,
    pub tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    /// GitHub Release ID once observed; `None` before the first exact
    /// observation. Never inferred from names.
    pub github_release_id: Option<String>,
    pub is_draft: bool,
    pub is_public: bool,
    /// The exact expected asset set. Declared once, never extended.
    pub expected_assets: Vec<GitHubReleaseExpectedAssetV1>,
    pub created_at_unix_seconds: u64,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
    pub entries: Vec<CargoAllowGitHubReleaseJournalEntryV1>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// One chained journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowGitHubReleaseJournalEntryV1 {
    pub sequence: u64,
    pub previous_digest: String,
    pub entry_digest: String,
    pub kind: GitHubReleaseJournalEventV1,
    pub asset_name: Option<String>,
    pub actual_asset: Option<GitHubReleaseActualAssetV1>,
    pub provider_reachable: bool,
    pub at_unix_seconds: u64,
    pub reason: String,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
}

/// Caller-supplied construction inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubReleaseJournalInitV1 {
    pub journal_id: String,
    pub operation_id: String,
    pub operation_identity_digest: String,
    pub operation_class: GitHubReleaseJournalClassV1,
    pub authorization_digest: String,
    pub custody_digest: String,
    pub freeze_digest: String,
    pub repository: String,
    pub tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    pub expected_assets: Vec<GitHubReleaseExpectedAssetV1>,
    pub created_at_unix_seconds: u64,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
}

/// Caller-supplied append request. Sequence and chaining are assigned by the
/// journal, never the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubReleaseJournalAppendV1 {
    pub kind: GitHubReleaseJournalEventV1,
    pub asset_name: Option<String>,
    pub actual_asset: Option<GitHubReleaseActualAssetV1>,
    pub provider_reachable: bool,
    pub at_unix_seconds: u64,
    pub reason: String,
}

const CLAIM_BOUNDARY: &str = "This record owns one append-only GitHub Release transaction history for one exact final-release operation: draft creation, exact asset uploads, actual-state reconciliation, and one-way public finalization. It does not create, edit, publish, or delete a real GitHub Release, authorize the operation, or prove asset semantics independently.";

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
struct GitHubJournalEntryDigestInputV1<'a> {
    sequence: u64,
    previous_digest: &'a str,
    journal_id: &'a str,
    operation_id: &'a str,
    operation_identity_digest: &'a str,
    operation_class: GitHubReleaseJournalClassV1,
    authorization_digest: &'a str,
    custody_digest: &'a str,
    freeze_digest: &'a str,
    repository: &'a str,
    tag: &'a str,
    channel: &'a str,
    github_prerelease: bool,
    github_release_id: Option<&'a str>,
    is_draft: bool,
    is_public: bool,
    expected_assets: &'a [GitHubReleaseExpectedAssetV1],
    created_at_unix_seconds: u64,
    kind: GitHubReleaseJournalEventV1,
    asset_name: Option<&'a str>,
    actual_asset: Option<&'a GitHubReleaseActualAssetV1>,
    provider_reachable: bool,
    at_unix_seconds: u64,
    reason: &'a str,
    workflow: &'a str,
    run: &'a str,
    attempt: &'a str,
    job: &'a str,
}

fn validate_expected_asset(asset: &GitHubReleaseExpectedAssetV1) -> Result<(), &'static str> {
    if asset.asset_role.trim().is_empty()
        || asset.asset_name.trim().is_empty()
        || asset.producer_identity.trim().is_empty()
        || asset.attestation_identity.trim().is_empty()
    {
        return Err("expected assets require role, name, producer, and attestation identity");
    }
    if asset.size_bytes < 1 {
        return Err("expected assets require a non-empty size");
    }
    if !digest_shape(&asset.sha256_digest) {
        return Err("expected assets require a canonical SHA-256 digest");
    }
    Ok(())
}

fn validate_actual_asset(asset: &GitHubReleaseActualAssetV1) -> Result<(), &'static str> {
    if asset.asset_id.trim().is_empty() || asset.asset_name.trim().is_empty() {
        return Err("actual assets require provider ID and name");
    }
    if asset.size_bytes < 1 {
        return Err("actual assets require a non-empty size");
    }
    if !digest_shape(&asset.observed_sha256_digest) {
        return Err("actual assets require a canonical observed SHA-256 digest");
    }
    Ok(())
}

/// Begin a journal bound to one operation. Expected assets are declared once;
/// recovery journals are not a separate class here: incident lineage travels
/// in `ReleaseIncident` entries on the same record.
pub fn begin_github_release_journal_v1(
    init: GitHubReleaseJournalInitV1,
) -> Result<CargoAllowGitHubReleaseJournalV1, &'static str> {
    if init.journal_id.trim().is_empty() {
        return Err("journal requires operation and journal identity");
    }
    if init.operation_id.trim().is_empty() {
        return Err("journal requires an operation name");
    }
    for value in [
        init.operation_identity_digest.as_str(),
        init.authorization_digest.as_str(),
        init.custody_digest.as_str(),
        init.freeze_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("journal requires canonical operation identity digests");
        }
    }
    if init.repository.trim().is_empty()
        || init.tag.trim().is_empty()
        || init.channel.trim().is_empty()
    {
        return Err("journal requires repository, tag, and channel identity");
    }
    if init.expected_assets.is_empty() {
        return Err("journal requires a non-empty expected asset set");
    }
    for asset in &init.expected_assets {
        validate_expected_asset(asset)?;
    }
    let mut seen: Vec<&str> = Vec::new();
    for asset in &init.expected_assets {
        if seen.contains(&asset.asset_name.as_str()) {
            return Err("expected asset names must be unique");
        }
        seen.push(asset.asset_name.as_str());
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
    if init.created_at_unix_seconds < 1 {
        return Err("journal requires a positive construction time");
    }
    Ok(CargoAllowGitHubReleaseJournalV1 {
        schema_id: GITHUB_RELEASE_JOURNAL_SCHEMA_ID.to_string(),
        schema_version: GITHUB_RELEASE_JOURNAL_SCHEMA_VERSION,
        journal_id: init.journal_id,
        operation_id: init.operation_id,
        operation_identity_digest: init.operation_identity_digest,
        operation_class: init.operation_class,
        authorization_digest: init.authorization_digest,
        custody_digest: init.custody_digest,
        freeze_digest: init.freeze_digest,
        repository: init.repository,
        tag: init.tag,
        channel: init.channel,
        github_prerelease: init.github_prerelease,
        github_release_id: None,
        is_draft: false,
        is_public: false,
        expected_assets: init.expected_assets,
        created_at_unix_seconds: init.created_at_unix_seconds,
        workflow: init.workflow,
        run: init.run,
        attempt: init.attempt,
        job: init.job,
        entries: Vec::new(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_create_releases".to_string(),
            "does_not_upload_assets".to_string(),
            "does_not_authorize_operation".to_string(),
        ],
    })
}

/// Begin a journal bound to one canonical #3940 release operation. The init
/// digests must equal the canonical identity's authority fields; the identity
/// itself is revalidated so a digest-shaped foreign operation can never own
/// a journal.
pub fn begin_github_release_journal_for_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    mut init: GitHubReleaseJournalInitV1,
) -> Result<CargoAllowGitHubReleaseJournalV1, &'static str> {
    validate_release_operation_identity_v1(identity)
        .map_err(|_| "journal operation identity is not canonical")?;
    let class_agrees = matches!(
        (&identity.operation_class, &init.operation_class,),
        (
            CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            GitHubReleaseJournalClassV1::CleanFinalPublication,
        ) | (
            CargoAllowReleaseOperationClassV1::IncidentRecovery,
            GitHubReleaseJournalClassV1::IncidentRecovery,
        )
    );
    if !class_agrees {
        return Err("journal class must agree with the canonical operation class");
    }
    if init.authorization_digest != identity.authorization_digest
        || init.custody_digest != identity.custody_digest
    {
        return Err("journal authority fields must agree with the canonical operation");
    }
    init.operation_identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    begin_github_release_journal_v1(init)
}

fn journal_has(
    journal: &CargoAllowGitHubReleaseJournalV1,
    kind: GitHubReleaseJournalEventV1,
) -> bool {
    journal.entries.iter().any(|entry| entry.kind == kind)
}

fn expected_asset<'a>(
    journal: &'a CargoAllowGitHubReleaseJournalV1,
    name: &str,
) -> Option<&'a GitHubReleaseExpectedAssetV1> {
    journal
        .expected_assets
        .iter()
        .find(|asset| asset.asset_name == name)
}

/// Append one event. Draft intent precedes draft start; asset intent precedes
/// asset start; observations require their started request; finalization
/// requires the exact actual draft, every expected asset observed, and a
/// complete closeout; public observation requires finalization; completion
/// requires public observation. Incidents record without prerequisites so
/// they can never be blocked, and nothing appends after completion.
pub fn append_github_release_journal_event_v1(
    journal: &mut CargoAllowGitHubReleaseJournalV1,
    append: GitHubReleaseJournalAppendV1,
) -> Result<(), &'static str> {
    use GitHubReleaseJournalEventV1 as Event;
    if journal_has(journal, Event::OperationComplete) {
        return Err("completed journals accept no further events");
    }
    if append.at_unix_seconds < 1 {
        return Err("journal events require a positive observation time");
    }
    if append.reason.trim().is_empty() {
        return Err("journal events require a reason");
    }
    if let Some(name) = append.asset_name.as_deref() {
        if name.trim().is_empty() {
            return Err("journal asset references must name an asset");
        }
        if matches!(
            append.kind,
            Event::AssetUploadIntentDurable
                | Event::AssetUploadStarted
                | Event::AssetUploadResponseObserved
                | Event::AssetUploadResponseUnknown
                | Event::AssetObservedExact
        ) && expected_asset(journal, name).is_none()
        {
            return Err("asset events must name an expected asset");
        }
    }
    if let Some(actual) = append.actual_asset.as_ref() {
        validate_actual_asset(actual)?;
    }
    match append.kind {
        Event::DraftCreateIntentDurable => {
            if journal_has(journal, Event::DraftCreateIntentDurable) {
                return Err("draft intent is append-once for one operation");
            }
        }
        Event::DraftCreateStarted => {
            if !journal_has(journal, Event::DraftCreateIntentDurable)
                || journal_has(journal, Event::DraftCreateStarted)
            {
                return Err("draft start requires one durable intent");
            }
        }
        Event::DraftCreateResponseObserved | Event::DraftCreateResponseUnknown => {
            if !journal_has(journal, Event::DraftCreateStarted) {
                return Err("draft responses belong to a started draft creation only");
            }
        }
        Event::DraftObservedExact => {
            if !journal_has(journal, Event::DraftCreateStarted) {
                return Err("draft observation requires a started draft creation");
            }
            if journal.github_release_id.is_none() {
                return Err("draft observation requires the observed release identity");
            }
        }
        Event::AssetUploadIntentDurable => {
            if !journal_has(journal, Event::DraftObservedExact) {
                return Err("asset intent requires an observed exact draft");
            }
        }
        Event::AssetUploadStarted => {
            let name = append
                .asset_name
                .as_deref()
                .ok_or("asset start requires an asset")?;
            if !journal.entries.iter().any(|entry| {
                entry.kind == Event::AssetUploadIntentDurable
                    && entry.asset_name.as_deref() == Some(name)
            }) {
                return Err("asset start requires its durable intent");
            }
            if journal.entries.iter().any(|entry| {
                entry.kind == Event::AssetUploadStarted && entry.asset_name.as_deref() == Some(name)
            }) {
                return Err(
                    "an asset upload starts once; unknown responses reconcile by observation, never by re-upload",
                );
            }
        }
        Event::AssetUploadResponseObserved | Event::AssetUploadResponseUnknown => {
            let name = append
                .asset_name
                .as_deref()
                .ok_or("asset responses belong to a started upload only")?;
            if !journal.entries.iter().any(|entry| {
                entry.kind == Event::AssetUploadStarted && entry.asset_name.as_deref() == Some(name)
            }) {
                return Err("asset responses belong to a started upload only");
            }
        }
        Event::AssetObservedExact => {
            let name = append
                .asset_name
                .as_deref()
                .ok_or("asset observation requires an asset")?;
            let expected = expected_asset(journal, name)
                .ok_or("asset observation requires an expected asset")?;
            let actual = append
                .actual_asset
                .as_ref()
                .ok_or("asset observation requires the actual observed asset")?;
            if actual.asset_name != expected.asset_name
                || actual.size_bytes != expected.size_bytes
                || actual.observed_sha256_digest != expected.sha256_digest
            {
                return Err("asset observation bytes must equal the expected asset exactly");
            }
            if !journal.entries.iter().any(|entry| {
                entry.kind == Event::AssetUploadStarted && entry.asset_name.as_deref() == Some(name)
            }) {
                return Err("asset observation requires a started upload");
            }
        }
        Event::ExtraAssetObserved => {
            let actual = append
                .actual_asset
                .as_ref()
                .ok_or("extra asset observation requires the actual observed asset")?;
            validate_actual_asset(actual)?;
            if append.asset_name.as_deref() != Some(actual.asset_name.as_str()) {
                return Err("extra asset observation must name its actual asset");
            }
            if expected_asset(journal, &actual.asset_name).is_some() {
                return Err("expected assets are observed exactly, never noted as extras");
            }
            if journal.entries.iter().any(|entry| {
                entry.kind == Event::ExtraAssetObserved
                    && entry.asset_name.as_deref() == Some(actual.asset_name.as_str())
            }) {
                return Err("extra assets are observed once per operation");
            }
        }
        Event::ActualAssetSetReconciled => {
            if !journal.expected_assets.iter().all(|expected| {
                journal.entries.iter().any(|entry| {
                    entry.kind == Event::AssetObservedExact
                        && entry.asset_name.as_deref() == Some(expected.asset_name.as_str())
                })
            }) {
                return Err("reconciliation requires every expected asset observed exact");
            }
            if journal_has(journal, Event::ExtraAssetObserved) {
                return Err("unmanifested observed assets block clean reconciliation");
            }
        }
        Event::CloseoutReceiptComplete => {
            if !journal_has(journal, Event::ActualAssetSetReconciled) {
                return Err("closeout requires a reconciled actual asset set");
            }
        }
        Event::FinalizeIntentDurable => {
            if !journal_has(journal, Event::CloseoutReceiptComplete) {
                return Err("finalization intent requires a complete closeout receipt");
            }
        }
        Event::FinalizeStarted => {
            if !journal_has(journal, Event::FinalizeIntentDurable)
                || journal_has(journal, Event::FinalizeStarted)
            {
                return Err("finalization start requires one durable intent");
            }
        }
        Event::FinalizeResponseObserved | Event::FinalizeResponseUnknown => {
            if !journal_has(journal, Event::FinalizeStarted) {
                return Err("finalization responses belong to a started finalization only");
            }
        }
        Event::PublicReleaseObservedExact => {
            if journal.github_prerelease {
                return Err("a prerelease channel never observes a stable public release");
            }
            if !journal_has(journal, Event::FinalizeStarted) {
                return Err("public observation requires started finalization");
            }
            if journal.is_public {
                return Err("public observation is append-once for one operation");
            }
            journal.is_public = true;
        }
        Event::ReleaseIncident => {}
        Event::OperationComplete => {
            if !journal_has(journal, Event::PublicReleaseObservedExact)
                && !journal_has(journal, Event::ReleaseIncident)
            {
                return Err("completion requires public observation or recorded incident lineage");
            }
        }
    }
    if append.kind == Event::PublicReleaseObservedExact {
        journal.is_public = true;
    }
    let sequence = journal.entries.len() as u64 + 1;
    let previous_digest = journal
        .entries
        .last()
        .map(|previous| previous.entry_digest.clone())
        .unwrap_or_else(|| GITHUB_RELEASE_JOURNAL_GENESIS_DIGEST.to_string());
    let input = GitHubJournalEntryDigestInputV1 {
        sequence,
        previous_digest: &previous_digest,
        journal_id: &journal.journal_id,
        operation_id: &journal.operation_id,
        operation_identity_digest: &journal.operation_identity_digest,
        operation_class: journal.operation_class,
        authorization_digest: &journal.authorization_digest,
        custody_digest: &journal.custody_digest,
        freeze_digest: &journal.freeze_digest,
        repository: &journal.repository,
        tag: &journal.tag,
        channel: &journal.channel,
        github_prerelease: journal.github_prerelease,
        github_release_id: journal.github_release_id.as_deref(),
        is_draft: journal.is_draft,
        is_public: journal.is_public,
        expected_assets: &journal.expected_assets,
        created_at_unix_seconds: journal.created_at_unix_seconds,
        kind: append.kind,
        asset_name: append.asset_name.as_deref(),
        actual_asset: append.actual_asset.as_ref(),
        provider_reachable: append.provider_reachable,
        at_unix_seconds: append.at_unix_seconds,
        reason: &append.reason,
        workflow: &journal.workflow,
        run: &journal.run,
        attempt: &journal.attempt,
        job: &journal.job,
    };
    let entry_digest = entry_digest(&input).map_err(|_| "journal entry digest failed")?;
    journal.entries.push(CargoAllowGitHubReleaseJournalEntryV1 {
        sequence,
        previous_digest,
        entry_digest,
        kind: append.kind,
        asset_name: append.asset_name,
        actual_asset: append.actual_asset,
        provider_reachable: append.provider_reachable,
        at_unix_seconds: append.at_unix_seconds,
        reason: append.reason,
        workflow: journal.workflow.clone(),
        run: journal.run.clone(),
        attempt: journal.attempt.clone(),
        job: journal.job.clone(),
    });
    Ok(())
}

/// Record the observed GitHub Release identity (ID, draft state) once an
/// exact observation names it. The ID is never inferred from names.
pub fn observe_github_release_identity_v1(
    journal: &mut CargoAllowGitHubReleaseJournalV1,
    github_release_id: &str,
    is_draft: bool,
) -> Result<(), &'static str> {
    if github_release_id.trim().is_empty() {
        return Err("release identity observation requires the provider ID");
    }
    if journal.github_release_id.is_some() {
        return Err("release identity is observed once per operation");
    }
    journal.github_release_id = Some(github_release_id.to_string());
    journal.is_draft = is_draft;
    Ok(())
}

/// Revalidate a loaded journal: identity, chain, digests, and transition law.
pub fn verify_github_release_journal_v1(
    journal: &CargoAllowGitHubReleaseJournalV1,
) -> Result<(), &'static str> {
    if journal.schema_id != GITHUB_RELEASE_JOURNAL_SCHEMA_ID
        || journal.schema_version != GITHUB_RELEASE_JOURNAL_SCHEMA_VERSION
        || journal.claim_boundary != CLAIM_BOUNDARY
    {
        return Err("journal uses an unsupported schema generation");
    }
    if journal.journal_id.trim().is_empty() || journal.operation_id.trim().is_empty() {
        return Err("journal requires operation and journal identity");
    }
    for value in [
        journal.operation_identity_digest.as_str(),
        journal.authorization_digest.as_str(),
        journal.custody_digest.as_str(),
        journal.freeze_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("journal requires canonical operation identity digests");
        }
    }
    let mut rebuilt = CargoAllowGitHubReleaseJournalV1 {
        schema_id: journal.schema_id.clone(),
        schema_version: journal.schema_version,
        journal_id: journal.journal_id.clone(),
        operation_id: journal.operation_id.clone(),
        operation_identity_digest: journal.operation_identity_digest.clone(),
        operation_class: journal.operation_class,
        authorization_digest: journal.authorization_digest.clone(),
        custody_digest: journal.custody_digest.clone(),
        freeze_digest: journal.freeze_digest.clone(),
        repository: journal.repository.clone(),
        tag: journal.tag.clone(),
        channel: journal.channel.clone(),
        github_prerelease: journal.github_prerelease,
        github_release_id: None,
        is_draft: false,
        is_public: false,
        expected_assets: journal.expected_assets.clone(),
        created_at_unix_seconds: journal.created_at_unix_seconds,
        workflow: journal.workflow.clone(),
        run: journal.run.clone(),
        attempt: journal.attempt.clone(),
        job: journal.job.clone(),
        entries: Vec::new(),
        claim_boundary: journal.claim_boundary.clone(),
        limitations: journal.limitations.clone(),
    };
    // Identity observations replay in entry order: entries that require the
    // observed release ID must see the same observation the original did.
    // The rebuilt journal adopts each observed identity exactly when the
    // original entry sequence first required it.
    let mut adopted_release_id: Option<String> = None;
    let mut adopted_is_draft = false;
    for entry in &journal.entries {
        if entry.kind == GitHubReleaseJournalEventV1::DraftObservedExact
            && adopted_release_id.is_none()
        {
            adopted_release_id = journal.github_release_id.clone();
            adopted_is_draft = journal.is_draft;
        }
        rebuilt.github_release_id = adopted_release_id.clone();
        rebuilt.is_draft = adopted_is_draft;
        let expected_kind = entry.kind;
        let append = GitHubReleaseJournalAppendV1 {
            kind: expected_kind,
            asset_name: entry.asset_name.clone(),
            actual_asset: entry.actual_asset.clone(),
            provider_reachable: entry.provider_reachable,
            at_unix_seconds: entry.at_unix_seconds,
            reason: entry.reason.clone(),
        };
        append_github_release_journal_event_v1(&mut rebuilt, append)?;
        let rebuilt_entry = rebuilt
            .entries
            .last()
            .ok_or("journal replay lost its entry")?;
        if rebuilt_entry.entry_digest != entry.entry_digest
            || rebuilt_entry.sequence != entry.sequence
            || rebuilt_entry.previous_digest != entry.previous_digest
        {
            return Err("journal entry digest does not match its canonical chain");
        }
    }
    // The terminal observed identity must equal the retained one.
    if rebuilt.github_release_id != journal.github_release_id
        || rebuilt.is_draft != journal.is_draft
        || rebuilt.is_public != journal.is_public
    {
        return Err("journal identity observations do not match the retained record");
    }
    Ok(())
}

/// Canonical JSON renderer for GitHub Release journals.
pub fn render_github_release_journal_v1(
    journal: &CargoAllowGitHubReleaseJournalV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(journal)
}
