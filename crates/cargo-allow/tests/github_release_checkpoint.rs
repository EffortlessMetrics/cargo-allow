//! Remotely durable GitHub Release checkpoints for total runner loss (#3933).
//!
//! Synthetic subjects only: no network access, no uploads, no provider API
//! calls, no credentials, and nothing leaves the process. Provider outcomes
//! are caller-supplied bytes and outage reports; the checkpoint classifies
//! them but never fetches them. These tests prove exact operation and
//! journal-prefix identity, monotonic linkage, immutable provider object
//! identity, producer trust, retention, schema parity, and the readback law:
//! provider success without readback is never clean.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use allow_report::{
    CargoAllowGitHubReleaseCheckpointV1, CargoAllowGitHubReleaseJournalV1,
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationPackageRowV1, GitHubCheckpointProviderOutcomeV1,
    GitHubReleaseCheckpointClassV1, GitHubReleaseCheckpointInitV1, GitHubReleaseCheckpointKindV1,
    GitHubReleaseCheckpointProducerV1, GitHubReleaseCheckpointProviderObjectV1,
    GitHubReleaseCheckpointProviderV1, GitHubReleaseCheckpointReadbackV1,
    GitHubReleaseCheckpointReadbackWitnessV1, GitHubReleaseExpectedAssetV1,
    GitHubReleaseJournalAppendV1, GitHubReleaseJournalClassV1, GitHubReleaseJournalEventV1,
    GitHubReleaseJournalInitV1, RELEASE_AUTHORIZATION_SELECTION, RELEASE_OPERATION_ASSET_SELECTION,
    append_github_release_journal_event_v1, begin_github_release_checkpoint_for_operation_v1,
    begin_github_release_checkpoint_v1, begin_github_release_journal_for_operation_v1,
    build_release_operation_identity_v1, digest_github_release_checkpoint_body_v1,
    observe_github_release_identity_v1, record_github_release_checkpoint_readback_v1,
    record_github_release_checkpoint_readback_with_witness_v1,
    release_operation_identity_digest_v1, render_github_release_checkpoint_v1,
    select_github_release_checkpoint_by_exact_identity_v1,
    verify_github_release_checkpoint_against_journal_v1,
};

const CREATED_AT: u64 = 1_786_200_000;
const NOW: u64 = 1_786_200_500;

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

fn journal_with_intent(
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
) -> Result<CargoAllowGitHubReleaseJournalV1, Box<dyn Error>> {
    let mut journal = begin_github_release_journal_for_operation_v1(
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
    .map_err(io::Error::other)?;
    let mut at = CREATED_AT;
    for kind in [
        GitHubReleaseJournalEventV1::DraftCreateIntentDurable,
        GitHubReleaseJournalEventV1::DraftCreateStarted,
        GitHubReleaseJournalEventV1::DraftCreateResponseObserved,
    ] {
        at += 10;
        append_github_release_journal_event_v1(
            &mut journal,
            GitHubReleaseJournalAppendV1 {
                kind,
                asset_name: None,
                actual_asset: None,
                provider_reachable: true,
                at_unix_seconds: at,
                reason: "synthetic".to_string(),
            },
        )
        .map_err(io::Error::other)?;
    }
    observe_github_release_identity_v1(&mut journal, "release-id-1", true)
        .map_err(io::Error::other)?;
    at += 10;
    append_github_release_journal_event_v1(
        &mut journal,
        GitHubReleaseJournalAppendV1 {
            kind: GitHubReleaseJournalEventV1::DraftObservedExact,
            asset_name: None,
            actual_asset: None,
            provider_reachable: true,
            at_unix_seconds: at,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    Ok(journal)
}

fn producer() -> GitHubReleaseCheckpointProducerV1 {
    GitHubReleaseCheckpointProducerV1 {
        workflow: "release".to_string(),
        run: "4242".to_string(),
        attempt: "1".to_string(),
        job: "github-release".to_string(),
        git_ref: "refs/tags/v0.2.0".to_string(),
        commit: "4d1b0fe10e82f506424f9dc7b7ff3bf81403191b".to_string(),
    }
}

fn checkpoint_init(
    journal: &CargoAllowGitHubReleaseJournalV1,
    sequence: u64,
    object_id: &str,
    kind: GitHubReleaseCheckpointKindV1,
) -> Result<GitHubReleaseCheckpointInitV1, Box<dyn Error>> {
    let head = journal
        .entries
        .last()
        .ok_or_else(|| io::Error::other("journal fixtures carry a prefix"))?;
    let init = GitHubReleaseCheckpointInitV1 {
        checkpoint_id: format!("github-checkpoint-{sequence:03}"),
        operation_id: "publish_cargo_allow_final_0_2_0".to_string(),
        operation_identity_digest: journal.operation_identity_digest.clone(),
        operation_class: GitHubReleaseCheckpointClassV1::CleanFinalPublication,
        authorization_digest: journal.authorization_digest.clone(),
        custody_digest: journal.custody_digest.clone(),
        freeze_digest: journal.freeze_digest.clone(),
        repository: journal.repository.clone(),
        tag: journal.tag.clone(),
        journal_head_sequence: head.sequence,
        journal_head_digest: head.entry_digest.clone(),
        checkpoint_sequence: sequence,
        kind,
        github_release_id: journal.github_release_id.clone(),
        gated_mutation: "draft-create".to_string(),
        provider: GitHubReleaseCheckpointProviderObjectV1 {
            provider: GitHubReleaseCheckpointProviderV1::GithubActionsArtifact,
            object_id: object_id.to_string(),
            object_name: "github-release-checkpoint".to_string(),
            object_digest: digest(0),
            object_size_bytes: 1,
        },
        producer: producer(),
        retention_days: 30,
        created_at_unix_seconds: CREATED_AT + sequence.saturating_sub(1) * 100,
        note: "synthetic".to_string(),
    };
    Ok(init)
}

/// Mirror the real producer protocol: render, bind the body digest, measure,
/// and converge the recorded size to the stored bytes by fixpoint.
fn store_checkpoint(
    checkpoint: &mut CargoAllowGitHubReleaseCheckpointV1,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let body_digest =
        digest_github_release_checkpoint_body_v1(checkpoint).map_err(io::Error::other)?;
    checkpoint.provider.object_digest = body_digest;
    for _ in 0..4 {
        let bytes = render_github_release_checkpoint_v1(checkpoint)
            .map_err(io::Error::other)?
            .into_bytes();
        let len = bytes.len() as u64;
        if checkpoint.provider.object_size_bytes == len {
            return Ok(bytes);
        }
        checkpoint.provider.object_size_bytes = len;
    }
    fail("checkpoint stored size must converge")
}

fn read_back(
    checkpoint: &mut CargoAllowGitHubReleaseCheckpointV1,
    bytes: Vec<u8>,
    at: u64,
) -> Result<Option<GitHubReleaseCheckpointReadbackWitnessV1>, Box<dyn Error>> {
    let (verdict, witness) = record_github_release_checkpoint_readback_with_witness_v1(
        checkpoint,
        GitHubCheckpointProviderOutcomeV1::Delivered(bytes),
        at,
    )
    .map_err(io::Error::other)?;
    require(
        verdict == GitHubReleaseCheckpointReadbackV1::Complete,
        "stored bytes must read back Complete",
    )?;
    Ok(witness)
}

#[test]
fn github_release_checkpoint_lifecycle() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-checkpoint-0001")?;
    let journal = journal_with_intent(&identity)?;
    // Canonical binding through the for_operation constructor.
    let mut first = begin_github_release_checkpoint_for_operation_v1(
        &identity,
        checkpoint_init(
            &journal,
            1,
            "artifact-1",
            GitHubReleaseCheckpointKindV1::PreMutationDurable,
        )?,
    )
    .map_err(io::Error::other)?;
    require(
        first.operation_identity_digest
            == release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?,
        "checkpoint must name the canonical operation identity digest",
    )?;
    let stored = store_checkpoint(&mut first)?;
    let witness = read_back(&mut first, stored, NOW)?
        .ok_or_else(|| io::Error::other("Complete readback must return a witness"))?;
    verify_github_release_checkpoint_against_journal_v1(
        &first,
        &witness,
        &journal,
        &producer(),
        NOW,
    )
    .map_err(io::Error::other)?;

    // Monotonic linkage: the post-observation checkpoint extends the chain.
    let mut second_init = checkpoint_init(
        &journal,
        2,
        "artifact-2",
        GitHubReleaseCheckpointKindV1::PostObservation,
    )?;
    second_init.created_at_unix_seconds = NOW + 10;
    let mut second =
        begin_github_release_checkpoint_v1(second_init, Some(&first)).map_err(io::Error::other)?;
    let stored = store_checkpoint(&mut second)?;
    read_back(&mut second, stored, NOW + 20)?;
    require(
        second.prior_checkpoint_digest.is_some(),
        "linked checkpoints must carry the predecessor digest",
    )?;

    // Control: the rendered checkpoint validates against its schema, and the
    // body digest binds both canonical operation digests.
    let body_before = digest_github_release_checkpoint_body_v1(&first).map_err(io::Error::other)?;
    let mut tampered = first.clone();
    tampered.operation_identity_digest = digest(999);
    require(
        digest_github_release_checkpoint_body_v1(&tampered).map_err(io::Error::other)?
            != body_before,
        "body digest must bind the canonical identity digest",
    )?;
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.github-release-checkpoint.v1.schema.json"),
        )?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("checkpoint schema compiles: {error}")))?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_github_release_checkpoint_v1(&first)?)?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!("stored checkpoint must validate: {error}"))
        })?;
    }
    Ok(())
}

#[test]
fn github_release_checkpoint_discovery_and_faults() -> Result<(), Box<dyn Error>> {
    let identity = canonical_identity("github-checkpoint-0002")?;
    let journal = journal_with_intent(&identity)?;
    let mut first = begin_github_release_checkpoint_for_operation_v1(
        &identity,
        checkpoint_init(
            &journal,
            1,
            "artifact-1",
            GitHubReleaseCheckpointKindV1::PreMutationDurable,
        )?,
    )
    .map_err(io::Error::other)?;
    let stored = store_checkpoint(&mut first)?;
    let first_witness = read_back(&mut first, stored, NOW)?
        .ok_or_else(|| io::Error::other("Complete readback must return a witness"))?;

    // Same name, wrong operation: exact object ID plus producer plus
    // operation name plus canonical digest selects; anything else never does.
    let foreign_identity = canonical_identity("github-checkpoint-0003")?;
    let mut foreign_init = checkpoint_init(
        &journal,
        1,
        "artifact-9",
        GitHubReleaseCheckpointKindV1::PreMutationDurable,
    )?;
    foreign_init.producer.run = "9999".to_string();
    foreign_init.operation_identity_digest =
        release_operation_identity_digest_v1(&foreign_identity).map_err(io::Error::other)?;
    let mut foreign =
        begin_github_release_checkpoint_v1(foreign_init, None).map_err(io::Error::other)?;
    let stored_foreign = store_checkpoint(&mut foreign)?;
    read_back(&mut foreign, stored_foreign, NOW)?;
    let crowded = vec![foreign.clone(), first.clone()];
    let picked = select_github_release_checkpoint_by_exact_identity_v1(
        &crowded,
        "artifact-1",
        &producer(),
        "publish_cargo_allow_final_0_2_0",
        &first.operation_identity_digest,
    )
    .map_err(io::Error::other)?;
    require(
        picked.is_some_and(|found| found.checkpoint_id == first.checkpoint_id),
        "discovery must return the exact-ID object on this operation",
    )?;
    require(
        select_github_release_checkpoint_by_exact_identity_v1(
            &crowded,
            "artifact-1",
            &producer(),
            "publish_cargo_allow_final_0_2_0",
            &foreign.operation_identity_digest,
        )
        .map_err(io::Error::other)?
        .is_none(),
        "same name and producer on the wrong operation must never resolve",
    )?;
    require(
        select_github_release_checkpoint_by_exact_identity_v1(
            &crowded,
            "github-release-checkpoint",
            &producer(),
            "publish_cargo_allow_final_0_2_0",
            &first.operation_identity_digest,
        )
        .map_err(io::Error::other)?
        .is_none(),
        "bare object names must never resolve to a checkpoint",
    )?;

    // Stale: older same-operation bytes report their age, never current.
    let mut older = first.clone();
    older.checkpoint_sequence = 0;
    let older_bytes = serde_json::to_vec_pretty(&older)?;
    let stale = record_github_release_checkpoint_readback_v1(
        &mut first.clone(),
        GitHubCheckpointProviderOutcomeV1::Delivered(older_bytes),
        NOW + 10,
    )
    .map_err(io::Error::other)?;
    require(
        stale == GitHubReleaseCheckpointReadbackV1::Stale,
        "older same-operation bytes must read back Stale",
    )?;

    // Mismatch: write succeeded but readback bytes differ.
    let mut tampered = first.clone();
    tampered.note = "tampered".to_string();
    let tampered_bytes = serde_json::to_vec_pretty(&tampered)?;
    let mismatch = record_github_release_checkpoint_readback_v1(
        &mut first.clone(),
        GitHubCheckpointProviderOutcomeV1::Delivered(tampered_bytes),
        NOW + 10,
    )
    .map_err(io::Error::other)?;
    require(
        mismatch == GitHubReleaseCheckpointReadbackV1::Mismatch,
        "differing readback bytes must read back Mismatch",
    )?;

    // Provider outage is never absence.
    let outage = record_github_release_checkpoint_readback_v1(
        &mut first.clone(),
        GitHubCheckpointProviderOutcomeV1::Unavailable,
        NOW + 10,
    )
    .map_err(io::Error::other)?;
    require(
        outage == GitHubReleaseCheckpointReadbackV1::ProviderUnavailable,
        "provider outage must report as outage, never absence",
    )?;

    // Expired state: a checkpoint past retention never authorizes progress.
    require(
        verify_github_release_checkpoint_against_journal_v1(
            &first,
            &first_witness,
            &journal,
            &producer(),
            first.expires_at_unix_seconds,
        )
        .is_err(),
        "expired checkpoints must never authorize progress",
    )?;
    Ok(())
}
