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

fn sample_producer() -> ProducerIdentityV1 {
    ProducerIdentityV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow_path: ".github/workflows/release.yml".to_string(),
        git_ref: "refs/tags/v0.2.0".to_string(),
        run_id: 12345678,
        run_attempt: 1,
        job_id: "package-candidates".to_string(),
        commit_sha: "abcd1234abcd1234abcd1234abcd1234abcd1234".to_string(),
        tree_sha: "tree1234tree1234tree1234tree1234tree1234".to_string(),
        release_version: "0.2.0".to_string(),
        tool_name: "cargo-allow".to_string(),
        schema_id: "cargo-allow.exact-candidate-package-set.v1".to_string(),
        producer_generation: 1,
    }
}

fn sample_consumer() -> ConsumerContextV1 {
    ConsumerContextV1 {
        workflow_path: ".github/workflows/release.yml".to_string(),
        run_id: 12345678,
        job_id: "publish-crates".to_string(),
        requested_role: "package-bundle".to_string(),
        is_credential_bearing: true,
    }
}

fn sample_transfer() -> CargoAllowReleaseArtifactTransferV1 {
    CargoAllowReleaseArtifactTransferV1::new(allow_report::ArtifactTransferInitV1 {
        transfer_id: "xfer-001".to_string(),
        role: "package-bundle".to_string(),
        stable_artifact_id: "stable-pkg-001".to_string(),
        producer: sample_producer(),
        provider_id: "actions/upload-artifact@v4".to_string(),
        provider_artifact_name: "release-package-bundle".to_string(),
        files: vec![
            ArtifactTransferFileV1 {
                path: "allow-core-0.2.0.crate".to_string(),
                size_bytes: 1024,
                sha256: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            },
            ArtifactTransferFileV1 {
                path: "cargo-allow-0.2.0.crate".to_string(),
                size_bytes: 2048,
                sha256: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
            },
        ],
        semantic_payload_digest: Some("sha256:payload-digest-001".to_string()),
        trust_class: TrustClassV1::CleanRelease,
        untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
        created_at_utc: "2026-08-26T00:00:00Z".to_string(),
    })
}

#[test]
fn test_artifact_transfer_complete_clean() -> Result<(), io::Error> {
    let transfer = sample_transfer();
    let consumer = sample_consumer();
    let downloaded = vec![
        ActualDownloadedFileV1 {
            path: "allow-core-0.2.0.crate".to_string(),
            size_bytes: 1024,
            sha256: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        },
        ActualDownloadedFileV1 {
            path: "cargo-allow-0.2.0.crate".to_string(),
            size_bytes: 2048,
            sha256: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
        },
    ];

    let result = transfer.evaluate_transfer(
        &consumer,
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        &downloaded,
    );

    require(
        result == ArtifactTransferDispositionV1::Complete,
        "expected Complete disposition",
    )?;
    Ok(())
}

#[test]
fn test_artifact_transfer_missing_file() -> Result<(), io::Error> {
    let transfer = sample_transfer();
    let consumer = sample_consumer();
    let downloaded = vec![ActualDownloadedFileV1 {
        path: "allow-core-0.2.0.crate".to_string(),
        size_bytes: 1024,
        sha256: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
    }];

    let result = transfer.evaluate_transfer(
        &consumer,
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        &downloaded,
    );

    require(
        result == ArtifactTransferDispositionV1::Mismatch,
        "expected Mismatch disposition when file count differs",
    )?;
    Ok(())
}

#[test]
fn test_artifact_transfer_digest_mismatch() -> Result<(), io::Error> {
    let transfer = sample_transfer();
    let consumer = sample_consumer();
    let downloaded = vec![
        ActualDownloadedFileV1 {
            path: "allow-core-0.2.0.crate".to_string(),
            size_bytes: 1024,
            sha256: "sha256:tampered-digest".to_string(),
        },
        ActualDownloadedFileV1 {
            path: "cargo-allow-0.2.0.crate".to_string(),
            size_bytes: 2048,
            sha256: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
        },
    ];

    let result = transfer.evaluate_transfer(
        &consumer,
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        &downloaded,
    );

    require(
        result == ArtifactTransferDispositionV1::Mismatch,
        "expected Mismatch disposition on digest divergence",
    )?;
    Ok(())
}

#[test]
fn test_artifact_transfer_stale_commit() -> Result<(), io::Error> {
    let transfer = sample_transfer();
    let consumer = sample_consumer();
    let downloaded = vec![
        ActualDownloadedFileV1 {
            path: "allow-core-0.2.0.crate".to_string(),
            size_bytes: 1024,
            sha256: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        },
        ActualDownloadedFileV1 {
            path: "cargo-allow-0.2.0.crate".to_string(),
            size_bytes: 2048,
            sha256: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
        },
    ];

    let result =
        transfer.evaluate_transfer(&consumer, "new-commit-hash-different", "0.2.0", &downloaded);

    require(
        result == ArtifactTransferDispositionV1::Stale,
        "expected Stale disposition when producer commit does not match expected",
    )?;
    Ok(())
}

#[test]
fn test_artifact_transfer_untrusted_pr_to_credential_bearing() -> Result<(), io::Error> {
    let mut transfer = sample_transfer();
    transfer.trust_class = TrustClassV1::PullRequest;
    let consumer = sample_consumer(); // is_credential_bearing = true
    let downloaded = vec![
        ActualDownloadedFileV1 {
            path: "allow-core-0.2.0.crate".to_string(),
            size_bytes: 1024,
            sha256: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        },
        ActualDownloadedFileV1 {
            path: "cargo-allow-0.2.0.crate".to_string(),
            size_bytes: 2048,
            sha256: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
        },
    ];

    let result = transfer.evaluate_transfer(
        &consumer,
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        &downloaded,
    );

    require(
        result == ArtifactTransferDispositionV1::Untrusted,
        "expected Untrusted disposition when PR artifact enters credential-bearing job",
    )?;
    Ok(())
}

#[test]
fn test_artifact_transfer_injection_rejection() -> Result<(), io::Error> {
    let mut transfer = sample_transfer();
    transfer.transfer_id = "xfer;rm -rf /".to_string();
    let consumer = sample_consumer();
    let downloaded = vec![];

    let result = transfer.evaluate_transfer(
        &consumer,
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        &downloaded,
    );

    require(
        result == ArtifactTransferDispositionV1::InstrumentFailure,
        "expected InstrumentFailure on shell injection characters",
    )?;
    Ok(())
}

#[test]
fn test_artifact_transfer_serde_roundtrip() -> Result<(), io::Error> {
    let transfer = sample_transfer();
    let json = serde_json::to_string(&transfer).map_err(io::Error::other)?;
    let parsed: CargoAllowReleaseArtifactTransferV1 =
        serde_json::from_str(&json).map_err(io::Error::other)?;

    require(parsed == transfer, "expected exact deserialization match")?;
    Ok(())
}
