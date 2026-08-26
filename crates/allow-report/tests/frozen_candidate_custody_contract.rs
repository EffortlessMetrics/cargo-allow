use allow_report::{
    CandidateCustodyInitV1, CargoAllowFrozenCandidateCustodyV1, ConfidentialityClassV1,
    CustodyDispositionV1, CustodyFileV1, RetainedCustodyItemV1,
};
use std::io;

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if !condition {
        return Err(io::Error::other(message));
    }
    Ok(())
}

fn sample_items() -> Vec<RetainedCustodyItemV1> {
    vec![
        RetainedCustodyItemV1 {
            role: "PackageArchive".to_string(),
            artifact_id: "allow-core-0.2.0".to_string(),
            files: vec![CustodyFileV1 {
                path: "allow-core-0.2.0.crate".to_string(),
                size_bytes: 4096,
                sha256: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            }],
            storage_locator: "s3://releases/v0.2.0/allow-core-0.2.0.crate".to_string(),
            retention_expiry_utc: "2027-01-01T00:00:00Z".to_string(),
            readback_verified: true,
            readback_sha256: Some(
                "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            ),
            confidentiality_class: ConfidentialityClassV1::Public,
        },
        RetainedCustodyItemV1 {
            role: "FreezeReceipt".to_string(),
            artifact_id: "freeze-receipt-0.2.0".to_string(),
            files: vec![CustodyFileV1 {
                path: "candidate-freeze.receipt.json".to_string(),
                size_bytes: 2048,
                sha256: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
            }],
            storage_locator: "s3://releases/v0.2.0/candidate-freeze.receipt.json".to_string(),
            retention_expiry_utc: "2027-01-01T00:00:00Z".to_string(),
            readback_verified: true,
            readback_sha256: Some(
                "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
            ),
            confidentiality_class: ConfidentialityClassV1::Public,
        },
    ]
}

fn sample_custody() -> CargoAllowFrozenCandidateCustodyV1 {
    CargoAllowFrozenCandidateCustodyV1::new(CandidateCustodyInitV1 {
        custody_id: "custody-0.2.0-001".to_string(),
        candidate_version: "0.2.0".to_string(),
        git_commit: "abcd1234abcd1234abcd1234abcd1234abcd1234".to_string(),
        git_tree: "tree1234tree1234tree1234tree1234tree1234".to_string(),
        items: sample_items(),
        created_at_utc: "2026-08-26T00:00:00Z".to_string(),
    })
}

#[test]
fn test_custody_evaluation_complete_clean() -> Result<(), io::Error> {
    let custody = sample_custody();
    let disposition = custody.evaluate_custody(
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        "2026-08-26T12:00:00Z",
    );

    require(
        disposition == CustodyDispositionV1::Complete,
        "clean custody evaluate must yield Complete",
    )?;
    Ok(())
}

#[test]
fn test_custody_evaluation_stale_commit() -> Result<(), io::Error> {
    let custody = sample_custody();
    let disposition =
        custody.evaluate_custody("different-commit-hash", "0.2.0", "2026-08-26T12:00:00Z");

    require(
        disposition == CustodyDispositionV1::Stale,
        "stale commit evaluate must yield Stale",
    )?;
    Ok(())
}

#[test]
fn test_custody_evaluation_readback_mismatch() -> Result<(), io::Error> {
    let mut custody = sample_custody();
    if let Some(first) = custody.items.first_mut() {
        first.readback_sha256 = Some("sha256:tampered".to_string());
    }

    let disposition = custody.evaluate_custody(
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        "2026-08-26T12:00:00Z",
    );

    require(
        disposition == CustodyDispositionV1::Mismatch,
        "divergent readback sha256 must yield Mismatch",
    )?;
    Ok(())
}

#[test]
fn test_custody_evaluation_expiring_item() -> Result<(), io::Error> {
    let mut custody = sample_custody();
    if let Some(first) = custody.items.first_mut() {
        first.retention_expiry_utc = "2026-08-26T00:00:00Z".to_string();
    }

    let disposition = custody.evaluate_custody(
        "abcd1234abcd1234abcd1234abcd1234abcd1234",
        "0.2.0",
        "2026-08-26T12:00:00Z", // Current time is past expiry
    );

    require(
        disposition == CustodyDispositionV1::Expiring,
        "expired item must yield Expiring disposition",
    )?;
    Ok(())
}

#[test]
fn test_custody_serde_roundtrip() -> Result<(), io::Error> {
    let custody = sample_custody();
    let json = serde_json::to_string(&custody).map_err(io::Error::other)?;
    let parsed: CargoAllowFrozenCandidateCustodyV1 =
        serde_json::from_str(&json).map_err(io::Error::other)?;

    require(
        parsed == custody,
        "deserialized custody must match original",
    )?;
    Ok(())
}
