//! Unknown GitHub API response reconciliation (#3933).
//!
//! Synthetic subjects only: no network access, no release creation or
//! mutation, no credentials, and nothing leaves the process. Unknown
//! responses are reconciled by exact observation before any retry or
//! continuation: runner loss after acceptance reuses the exact remote state
//! instead of duplicating it, and a later green run can never erase an
//! earlier incident.

use std::error::Error;
use std::io;

use GitHubReleaseJournalEventV1 as Event;
use allow_report::{
    CargoAllowGitHubReleaseJournalV1, CargoAllowReleaseOperationAssetRowV1,
    CargoAllowReleaseOperationAuthorityKindV1, CargoAllowReleaseOperationClassV1,
    CargoAllowReleaseOperationIdentityInitV1, CargoAllowReleaseOperationPackageRowV1,
    GitHubReleaseActualAssetV1, GitHubReleaseExpectedAssetV1, GitHubReleaseJournalAppendV1,
    GitHubReleaseJournalClassV1, GitHubReleaseJournalEventV1, GitHubReleaseJournalInitV1,
    RELEASE_AUTHORIZATION_SELECTION, RELEASE_OPERATION_ASSET_SELECTION,
    append_github_release_journal_event_v1, begin_github_release_journal_for_operation_v1,
    build_release_operation_identity_v1, observe_github_release_identity_v1,
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

fn canonical_identity(
    nonce: &str,
) -> Result<allow_report::CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
    let packages = RELEASE_AUTHORIZATION_SELECTION
        .iter()
        .filter(|(_, _, _, shared)| !*shared)
        .enumerate()
        .map(|(index, (logical_id, package_name, version, _))| {
            CargoAllowReleaseOperationPackageRowV1 {
                logical_id: (*logical_id).to_string(),
                package_name: (*package_name).to_string(),
                package_version: (*version).to_string(),
                package_digest: digest(100 + index as u64),
            }
        })
        .collect();
    let assets = RELEASE_OPERATION_ASSET_SELECTION
        .iter()
        .enumerate()
        .map(
            |(index, (asset_id, asset_name))| CargoAllowReleaseOperationAssetRowV1 {
                asset_id: (*asset_id).to_string(),
                asset_name: (*asset_name).to_string(),
                asset_digest: digest(200 + index as u64),
            },
        )
        .collect();
    Ok(
        build_release_operation_identity_v1(CargoAllowReleaseOperationIdentityInitV1 {
            nonce: nonce.to_string(),
            operation_class: CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            authority_kind: CargoAllowReleaseOperationAuthorityKindV1::Clean,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            product: "cargo-allow".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            github_prerelease: false,
            freeze_digest: digest(1),
            final_evidence_graph_digest: digest(2),
            custody_digest: digest(3),
            replay_digest: digest(4),
            authorization_digest: digest(5),
            cargo_lock_digest: digest(6),
            topology_digest: digest(7),
            support_digest: digest(8),
            channel_digest: digest(9),
            packages,
            assets,
            workflow_digest: digest(10),
            action_inventory_digest: digest(11),
            live_controls_digest: digest(12),
            incident_predecessor_operation_digest: None,
            incident_predecessor_head_digest: None,
            one_run_scope: true,
            expires_at_unix_seconds: 1_800_000_000,
        })
        .map_err(io::Error::other)?,
    )
}

fn expected_asset() -> GitHubReleaseExpectedAssetV1 {
    GitHubReleaseExpectedAssetV1 {
        asset_role: "release-attachment".to_string(),
        asset_name: "cargo-allow.tar.gz".to_string(),
        size_bytes: 101,
        sha256_digest: digest(301),
        producer_identity: "release-publish".to_string(),
        attestation_identity: "attestation-1".to_string(),
    }
}

fn actual_asset() -> GitHubReleaseActualAssetV1 {
    GitHubReleaseActualAssetV1 {
        asset_id: "asset-id-1".to_string(),
        asset_name: "cargo-allow.tar.gz".to_string(),
        size_bytes: 101,
        observed_sha256_digest: digest(301),
    }
}

fn begin_journal(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
) -> Result<CargoAllowGitHubReleaseJournalV1, Box<dyn Error>> {
    Ok(begin_github_release_journal_for_operation_v1(
        identity,
        GitHubReleaseJournalInitV1 {
            journal_id: "github-journal-0-2-0-001".to_string(),
            operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
            operation_identity_digest: digest(0),
            operation_class: GitHubReleaseJournalClassV1::CleanFinalPublication,
            authorization_digest: identity.authorization_digest.clone(),
            custody_digest: identity.custody_digest.clone(),
            freeze_digest: digest(72),
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            github_prerelease: false,
            expected_assets: vec![expected_asset()],
            created_at_unix_seconds: CREATED_AT,
            workflow: "release".to_string(),
            run: "4242".to_string(),
            attempt: "1".to_string(),
            job: "github-release".to_string(),
        },
    )
    .map_err(io::Error::other)?)
}

fn append(
    journal: &mut CargoAllowGitHubReleaseJournalV1,
    kind: GitHubReleaseJournalEventV1,
    asset_name: Option<&str>,
    actual: Option<GitHubReleaseActualAssetV1>,
    at: u64,
) -> Result<(), Box<dyn Error>> {
    Ok(append_github_release_journal_event_v1(
        journal,
        GitHubReleaseJournalAppendV1 {
            kind,
            asset_name: asset_name.map(str::to_string),
            actual_asset: actual,
            provider_reachable: true,
            at_unix_seconds: at,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?)
}

#[test]
fn unknown_draft_response_reuses_exact_remote_state() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-unknown-0001")?;
    let mut journal = begin_journal(&identity)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    // Negative 1: runner loss after GitHub accepted draft creation but
    // before response logging. The unknown reconciles by observing the
    // exact draft: no second draft is ever created.
    append(
        &mut journal,
        Event::DraftCreateIntentDurable,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::DraftCreateStarted, None, None, next())?;
    append(
        &mut journal,
        Event::DraftCreateResponseUnknown,
        None,
        None,
        next(),
    )?;
    require(
        append(
            &mut journal,
            Event::DraftCreateIntentDurable,
            None,
            None,
            next(),
        )
        .is_err(),
        "an unknown draft response must never authorize a second draft",
    )?;
    observe_github_release_identity_v1(&mut journal, "release-id-1", true)
        .map_err(io::Error::other)?;
    append(&mut journal, Event::DraftObservedExact, None, None, next())?;
    require(
        journal.github_release_id.as_deref() == Some("release-id-1"),
        "the unknown draft must reconcile to the exact accepted draft",
    )?;
    Ok(())
}

#[test]
fn unknown_asset_response_reconciles_without_reupload() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-unknown-0002")?;
    let mut journal = begin_journal(&identity)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    append(
        &mut journal,
        Event::DraftCreateIntentDurable,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::DraftCreateStarted, None, None, next())?;
    append(
        &mut journal,
        Event::DraftCreateResponseObserved,
        None,
        None,
        next(),
    )?;
    observe_github_release_identity_v1(&mut journal, "release-id-1", true)
        .map_err(io::Error::other)?;
    append(&mut journal, Event::DraftObservedExact, None, None, next())?;
    append(
        &mut journal,
        Event::AssetUploadIntentDurable,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::AssetUploadStarted,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    // Negative 3: asset upload accepted but response lost. The unknown
    // reconciles by exact observation of the same bytes, never by upload.
    append(
        &mut journal,
        Event::AssetUploadResponseUnknown,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::AssetObservedExact,
        Some("cargo-allow.tar.gz"),
        Some(actual_asset()),
        next(),
    )?;
    require(
        journal
            .entries
            .iter()
            .filter(|entry| {
                entry.kind == Event::AssetUploadStarted
                    && entry.asset_name.as_deref() == Some("cargo-allow.tar.gz")
            })
            .count()
            == 1,
        "the accepted-but-unrecorded upload must start exactly once",
    )?;
    // Negative 10: runner loss after finalization acceptance but before
    // response logging reconciles the same way: observe, never re-finalize.
    append(
        &mut journal,
        Event::ActualAssetSetReconciled,
        None,
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::CloseoutReceiptComplete,
        None,
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::FinalizeIntentDurable,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::FinalizeStarted, None, None, next())?;
    append(
        &mut journal,
        Event::FinalizeResponseUnknown,
        None,
        None,
        next(),
    )?;
    require(
        append(&mut journal, Event::FinalizeStarted, None, None, next()).is_err(),
        "an unknown finalization response must never authorize a second finalization",
    )?;
    append(
        &mut journal,
        Event::PublicReleaseObservedExact,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::OperationComplete, None, None, next())?;
    Ok(())
}

#[test]
fn later_green_run_preserves_incident_lineage() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-unknown-0003")?;
    let mut journal = begin_journal(&identity)?;
    let mut at = CREATED_AT;
    let mut next = || {
        at += 10;
        at
    };
    // Negative 15: a later green run can never erase an earlier incident.
    // Record the incident first, then drive the full lifecycle to completion.
    append(&mut journal, Event::ReleaseIncident, None, None, next())?;
    append(
        &mut journal,
        Event::DraftCreateIntentDurable,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::DraftCreateStarted, None, None, next())?;
    append(
        &mut journal,
        Event::DraftCreateResponseObserved,
        None,
        None,
        next(),
    )?;
    observe_github_release_identity_v1(&mut journal, "release-id-1", true)
        .map_err(io::Error::other)?;
    append(&mut journal, Event::DraftObservedExact, None, None, next())?;
    append(
        &mut journal,
        Event::AssetUploadIntentDurable,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::AssetUploadStarted,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::AssetUploadResponseObserved,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::AssetObservedExact,
        Some("cargo-allow.tar.gz"),
        Some(actual_asset()),
        next(),
    )?;
    append(
        &mut journal,
        Event::ActualAssetSetReconciled,
        None,
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::CloseoutReceiptComplete,
        None,
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::FinalizeIntentDurable,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::FinalizeStarted, None, None, next())?;
    append(
        &mut journal,
        Event::FinalizeResponseObserved,
        None,
        None,
        next(),
    )?;
    append(
        &mut journal,
        Event::PublicReleaseObservedExact,
        None,
        None,
        next(),
    )?;
    append(&mut journal, Event::OperationComplete, None, None, next())?;
    require(
        journal
            .entries
            .iter()
            .any(|entry| entry.kind == Event::ReleaseIncident),
        "completion must preserve the recorded incident lineage",
    )?;
    allow_report::verify_github_release_journal_v1(&journal).map_err(io::Error::other)?;
    Ok(())
}
