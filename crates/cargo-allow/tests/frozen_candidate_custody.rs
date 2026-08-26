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

fn build_thirteen_crate_custody() -> CargoAllowFrozenCandidateCustodyV1 {
    let crates = [
        "allow-core",
        "allow-policy",
        "allow-policy-legacy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-diff",
        "cargo-allow",
    ];

    let items = crates
        .iter()
        .map(|name| RetainedCustodyItemV1 {
            role: "PackageArchive".to_string(),
            artifact_id: format!("{name}-0.2.0"),
            files: vec![CustodyFileV1 {
                path: format!("packages/{name}-0.2.0.crate"),
                size_bytes: 8192,
                sha256: format!("sha256:digest-for-{name}"),
            }],
            storage_locator: format!("s3://release-custody-2026/0.2.0/{name}-0.2.0.crate"),
            retention_expiry_utc: "2027-01-01T00:00:00Z".to_string(),
            readback_verified: true,
            readback_sha256: Some(format!("sha256:digest-for-{name}")),
            confidentiality_class: ConfidentialityClassV1::Public,
        })
        .collect();

    CargoAllowFrozenCandidateCustodyV1::new(CandidateCustodyInitV1 {
        custody_id: "candidate-custody-0.2.0-clean".to_string(),
        candidate_version: "0.2.0".to_string(),
        git_commit: "99887766554433221100aabbccddeeff00112233".to_string(),
        git_tree: "11223344556677889900aabbccddeeff00112233".to_string(),
        items,
        created_at_utc: "2026-08-26T06:00:00Z".to_string(),
    })
}

#[test]
fn test_frozen_candidate_custody_retained_package_graph() -> Result<(), io::Error> {
    let custody = build_thirteen_crate_custody();

    let disposition = custody.evaluate_custody(
        "99887766554433221100aabbccddeeff00112233",
        "0.2.0",
        "2026-08-26T12:00:00Z",
    );

    require(
        disposition == CustodyDispositionV1::Complete,
        "full retained candidate package graph must evaluate as Complete",
    )?;
    Ok(())
}

#[test]
fn test_frozen_candidate_custody_detects_unverified_readback() -> Result<(), io::Error> {
    let mut custody = build_thirteen_crate_custody();
    if let Some(third) = custody.items.get_mut(2) {
        third.readback_verified = false;
    }

    let disposition = custody.evaluate_custody(
        "99887766554433221100aabbccddeeff00112233",
        "0.2.0",
        "2026-08-26T12:00:00Z",
    );

    require(
        disposition == CustodyDispositionV1::Missing,
        "unverified readback must fail as Missing",
    )?;
    Ok(())
}

#[test]
fn test_frozen_candidate_custody_version_mismatch() -> Result<(), io::Error> {
    let custody = build_thirteen_crate_custody();

    let disposition = custody.evaluate_custody(
        "99887766554433221100aabbccddeeff00112233",
        "0.3.0",
        "2026-08-26T12:00:00Z",
    );

    require(
        disposition == CustodyDispositionV1::Mismatch,
        "divergent version expectation must yield Mismatch",
    )?;
    Ok(())
}
