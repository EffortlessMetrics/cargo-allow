use allow_report::{
    ActualDownloadedFileV1, ArtifactTransferDispositionV1, ArtifactTransferFileV1,
    CargoAllowReleaseArtifactTransferV1, ConsumerContextV1, ProducerIdentityV1, TrustClassV1,
    UntrustedInputPostureV1,
};
use std::io;

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if !condition {
        return Err(io::Error::other(message));
    }
    Ok(())
}

fn make_valid_transfer(trust: TrustClassV1) -> CargoAllowReleaseArtifactTransferV1 {
    CargoAllowReleaseArtifactTransferV1::new(allow_report::ArtifactTransferInitV1 {
        transfer_id: "transfer-20260826-001".to_string(),
        role: "candidate-crate-bundle".to_string(),
        stable_artifact_id: "stable-bundle-id-42".to_string(),
        producer: ProducerIdentityV1 {
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            workflow_path: ".github/workflows/release.yml".to_string(),
            git_ref: "refs/tags/v0.2.0".to_string(),
            run_id: 99887766,
            run_attempt: 1,
            job_id: "package-set-builder".to_string(),
            commit_sha: "11223344556677889900aabbccddeeff00112233".to_string(),
            tree_sha: "aabbccddeeff0011223344556677889900aabbcc".to_string(),
            release_version: "0.2.0".to_string(),
            tool_name: "cargo-allow".to_string(),
            schema_id: "cargo-allow.exact-candidate-package-set.v1".to_string(),
            producer_generation: 1,
        },
        provider_id: "actions/upload-artifact@v4".to_string(),
        provider_artifact_name: "release-candidates-0.2.0".to_string(),
        files: vec![
            ArtifactTransferFileV1 {
                path: "packages/allow-core-0.2.0.crate".to_string(),
                size_bytes: 5120,
                sha256: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            },
            ArtifactTransferFileV1 {
                path: "packages/allow-policy-0.2.0.crate".to_string(),
                size_bytes: 8192,
                sha256: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            },
            ArtifactTransferFileV1 {
                path: "packages/cargo-allow-0.2.0.crate".to_string(),
                size_bytes: 32768,
                sha256: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
            },
        ],
        semantic_payload_digest: Some("sha256:bundle-manifest-digest-001".to_string()),
        trust_class: trust,
        untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
        created_at_utc: "2026-08-26T04:00:00Z".to_string(),
    })
}

fn valid_downloaded_files() -> Vec<ActualDownloadedFileV1> {
    vec![
        ActualDownloadedFileV1 {
            path: "packages/allow-core-0.2.0.crate".to_string(),
            size_bytes: 5120,
            sha256: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
        },
        ActualDownloadedFileV1 {
            path: "packages/allow-policy-0.2.0.crate".to_string(),
            size_bytes: 8192,
            sha256: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
        },
        ActualDownloadedFileV1 {
            path: "packages/cargo-allow-0.2.0.crate".to_string(),
            size_bytes: 32768,
            sha256: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
        },
    ]
}

#[test]
fn test_release_artifact_transfer_end_to_end_validation() -> Result<(), io::Error> {
    let transfer = make_valid_transfer(TrustClassV1::CleanRelease);
    let consumer = ConsumerContextV1 {
        workflow_path: ".github/workflows/release.yml".to_string(),
        run_id: 99887766,
        job_id: "publish-crates-io".to_string(),
        requested_role: "candidate-crate-bundle".to_string(),
        is_credential_bearing: true,
    };
    let downloaded = valid_downloaded_files();

    let disposition = transfer.evaluate_transfer(
        &consumer,
        "11223344556677889900aabbccddeeff00112233",
        "0.2.0",
        &downloaded,
    );

    require(
        disposition == ArtifactTransferDispositionV1::Complete,
        "valid clean release artifact transfer must yield Complete disposition",
    )?;
    Ok(())
}

#[test]
fn test_release_artifact_transfer_untrusted_fork_blocked_from_release() -> Result<(), io::Error> {
    let transfer = make_valid_transfer(TrustClassV1::Fork);
    let consumer = ConsumerContextV1 {
        workflow_path: ".github/workflows/release.yml".to_string(),
        run_id: 99887766,
        job_id: "publish-crates-io".to_string(),
        requested_role: "candidate-crate-bundle".to_string(),
        is_credential_bearing: true,
    };
    let downloaded = valid_downloaded_files();

    let disposition = transfer.evaluate_transfer(
        &consumer,
        "11223344556677889900aabbccddeeff00112233",
        "0.2.0",
        &downloaded,
    );

    require(
        disposition == ArtifactTransferDispositionV1::Untrusted,
        "fork artifact transfer must be Untrusted when entering credential job",
    )?;
    Ok(())
}

#[test]
fn test_release_artifact_transfer_tampered_payload_blocked() -> Result<(), io::Error> {
    let transfer = make_valid_transfer(TrustClassV1::CleanRelease);
    let consumer = ConsumerContextV1 {
        workflow_path: ".github/workflows/release.yml".to_string(),
        run_id: 99887766,
        job_id: "publish-crates-io".to_string(),
        requested_role: "candidate-crate-bundle".to_string(),
        is_credential_bearing: true,
    };
    let mut downloaded = valid_downloaded_files();
    if let Some(first) = downloaded.get_mut(0) {
        first.sha256 = "sha256:tampered".to_string();
    }

    let disposition = transfer.evaluate_transfer(
        &consumer,
        "11223344556677889900aabbccddeeff00112233",
        "0.2.0",
        &downloaded,
    );

    require(
        disposition == ArtifactTransferDispositionV1::Mismatch,
        "tampered artifact byte digest must yield Mismatch disposition",
    )?;
    Ok(())
}
