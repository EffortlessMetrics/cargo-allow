//! Append-only GitHub Release transaction journal (#3933).
//!
//! Synthetic subjects only: no network access, no release creation or
//! mutation, no credentials, and nothing leaves the process. These tests
//! prove the crash-consistent mutation history: draft intent before start,
//! exact asset observations, actual-set reconciliation, closeout-gated
//! finalization, one-way public observation, and append-only incident
//! lineage that no later green run can erase.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    release_operation_identity_digest_v1, render_github_release_journal_v1,
    verify_github_release_journal_v1,
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

fn expected_asset(name: &str, n: u64) -> GitHubReleaseExpectedAssetV1 {
    GitHubReleaseExpectedAssetV1 {
        asset_role: "release-attachment".to_string(),
        asset_name: name.to_string(),
        size_bytes: 100 + n,
        sha256_digest: digest(300 + n),
        producer_identity: "release-publish".to_string(),
        attestation_identity: "attestation-1".to_string(),
    }
}

fn actual_asset(name: &str, n: u64) -> GitHubReleaseActualAssetV1 {
    GitHubReleaseActualAssetV1 {
        asset_id: format!("asset-id-{n}"),
        asset_name: name.to_string(),
        size_bytes: 100 + n,
        observed_sha256_digest: digest(300 + n),
    }
}

fn begin_init(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
) -> GitHubReleaseJournalInitV1 {
    begin_init_prerelease(identity, false)
}

fn begin_init_prerelease(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
    github_prerelease: bool,
) -> GitHubReleaseJournalInitV1 {
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
        github_prerelease,
        expected_assets: vec![expected_asset("cargo-allow.tar.gz", 1)],
        created_at_unix_seconds: CREATED_AT,
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "github-release".to_string(),
    }
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

fn full_lifecycle(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
) -> Result<CargoAllowGitHubReleaseJournalV1, Box<dyn Error>> {
    full_lifecycle_prerelease(identity, false)
}

fn full_lifecycle_prerelease(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
    github_prerelease: bool,
) -> Result<CargoAllowGitHubReleaseJournalV1, Box<dyn Error>> {
    let mut journal = begin_github_release_journal_for_operation_v1(
        identity,
        begin_init_prerelease(identity, github_prerelease),
    )
    .map_err(io::Error::other)?;
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
        Some(actual_asset("cargo-allow.tar.gz", 1)),
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
    Ok(journal)
}

#[test]
fn github_release_journal_full_lifecycle() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-journal-0001")?;
    let journal = full_lifecycle(&identity)?;
    require(
        journal.entries.len() == 15,
        "the full mutation lifecycle must append exactly fifteen entries",
    )?;
    require(
        journal.is_public && journal.github_release_id.as_deref() == Some("release-id-1"),
        "completion must retain the observed public release identity",
    )?;
    verify_github_release_journal_v1(&journal).map_err(io::Error::other)?;
    require(
        journal.operation_identity_digest
            == release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?,
        "the journal must carry the canonical operation identity digest",
    )?;

    // Control: the rendered journal validates against its schema.
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.github-release-journal.v1.schema.json"),
        )?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("journal schema compiles: {error}")))?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_github_release_journal_v1(&journal)?)?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!("rendered journal violates schema: {error}"))
        })?;
    }
    Ok(())
}

#[test]
fn github_release_journal_rejects_duplicate_draft_and_reupload() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-journal-0002")?;
    let mut journal =
        begin_github_release_journal_for_operation_v1(&identity, begin_init(&identity))
            .map_err(io::Error::other)?;
    // Negative 2: a later run must not create a second draft.
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
    require(
        append(
            &mut journal,
            Event::DraftCreateIntentDurable,
            None,
            None,
            next(),
        )
        .is_err(),
        "draft intent is append-once: no second draft",
    )?;
    append(&mut journal, Event::DraftCreateStarted, None, None, next())?;
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
        Event::AssetUploadResponseUnknown,
        Some("cargo-allow.tar.gz"),
        None,
        next(),
    )?;
    // Negative 5: one asset is never uploaded twice after an unknown
    // response; the unknown reconciles by observation, not by re-upload.
    require(
        append(
            &mut journal,
            Event::AssetUploadStarted,
            Some("cargo-allow.tar.gz"),
            None,
            next(),
        )
        .is_err(),
        "an asset upload starts once; unknown responses reconcile by observation",
    )?;
    // The unknown reconciles by exact observation of the same bytes.
    append(
        &mut journal,
        Event::AssetObservedExact,
        Some("cargo-allow.tar.gz"),
        Some(actual_asset("cargo-allow.tar.gz", 1)),
        next(),
    )?;
    Ok(())
}

#[test]
fn github_release_journal_rejects_wrong_extra_and_premature_steps() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-journal-0003")?;
    let mut journal =
        begin_github_release_journal_for_operation_v1(&identity, begin_init(&identity))
            .map_err(io::Error::other)?;
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
    // Negative 4: same-name wrong-byte actuals fail observation; the wrong
    // bytes are never deleted or replaced under a clean operation.
    let mut wrong = actual_asset("cargo-allow.tar.gz", 1);
    wrong.observed_sha256_digest = digest(999);
    require(
        append(
            &mut journal,
            Event::AssetObservedExact,
            Some("cargo-allow.tar.gz"),
            Some(wrong),
            next(),
        )
        .is_err(),
        "same-name wrong-byte assets must fail exact observation",
    )?;
    // Negative 7: unexpected extra executables are explicit and block
    // clean reconciliation.
    require(
        append(
            &mut journal,
            Event::ExtraAssetObserved,
            Some("unexpected-tool.tar.gz"),
            Some(actual_asset("unexpected-tool.tar.gz", 9)),
            next(),
        )
        .is_ok(),
        "extra assets must record explicitly",
    )?;
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
        Some(actual_asset("cargo-allow.tar.gz", 1)),
        next(),
    )?;
    require(
        append(
            &mut journal,
            Event::ActualAssetSetReconciled,
            None,
            None,
            next(),
        )
        .is_err(),
        "unmanifested extras must block clean reconciliation",
    )?;
    // Negative 9: finalization never begins on an incomplete closeout.
    require(
        append(
            &mut journal,
            Event::FinalizeIntentDurable,
            None,
            None,
            next(),
        )
        .is_err(),
        "finalization requires a complete closeout receipt",
    )?;
    // Negative 11: a prerelease channel never observes a stable public release.
    let prerelease_identity = canonical_identity("github-journal-0004")?;
    require(
        full_lifecycle_prerelease(&prerelease_identity, true).is_err(),
        "a prerelease journal must never complete a stable public release",
    )?;
    Ok(())
}
