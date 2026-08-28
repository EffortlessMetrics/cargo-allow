use allow_report::{
    IsolatedInstallGraphComparisonV2, IsolatedInstallPackageRowV2, IsolatedInstallPayloadV2,
    IsolatedInstallResultV2, validate_isolated_install_v2,
};

/// Golden receipt: a clean isolated install classifies Complete and carries
/// no private path data.
#[test]
fn clean_isolated_install_receipt_validates_complete() {
    let payload = receipt_payload();
    let validation = validate_isolated_install_v2(&payload);
    assert_eq!(
        validation.result,
        IsolatedInstallResultV2::Complete,
        "golden receipt rejected: {validation:?}"
    );
}

/// The receipt must fail closed when the source checkout denial is absent,
/// the graph is dirty, or a private path leaks into a portable field.
#[test]
fn receipt_failures_are_classified_fail_closed() {
    let mut fallback = receipt_payload();
    fallback.source_checkout_denied = false;
    let validation = validate_isolated_install_v2(&fallback);
    assert_eq!(
        validation.result,
        IsolatedInstallResultV2::SourceFallbackDetected
    );

    let mut dirty = receipt_payload();
    dirty.graph_comparison = IsolatedInstallGraphComparisonV2 {
        expected_packages: 13,
        matched_packages: 12,
        unexpected_packages: vec!["intent-model".to_string()],
        missing_packages: vec![],
        version_mismatches: vec!["allow-core expected 0.2.0-rc.1 resolved 0.2.0-rc.2".to_string()],
        path_sources: vec![],
    };
    let validation = validate_isolated_install_v2(&dirty);
    assert_eq!(validation.result, IsolatedInstallResultV2::GraphMismatch);

    let mut leaky = receipt_payload();
    leaky.external_cache_identity = "/home/runner/work/cargo-allow/cache".to_string();
    let validation = validate_isolated_install_v2(&leaky);
    assert_eq!(
        validation.result,
        IsolatedInstallResultV2::PathLeakInReceipt
    );
}

/// Mixed-version identity must be retained row by row: the shared substrate
/// rows stay 0.1.0 while the cargo-allow family stays 0.2.0-rc.1.
#[test]
fn receipt_retains_mixed_version_rows() {
    let payload = receipt_payload();
    assert_eq!(payload.package_rows.len(), 13);
    for row in &payload.package_rows {
        let expected = if row.package_name.starts_with("effortless-") {
            "0.1.0"
        } else {
            "0.2.0-rc.1"
        };
        assert_eq!(row.package_version, expected, "row {}", row.package_name);
    }
}

fn receipt_payload() -> IsolatedInstallPayloadV2 {
    let rows = [
        ("cargo-allow", "0.2.0-rc.1"),
        ("allow-core", "0.2.0-rc.1"),
        ("allow-policy", "0.2.0-rc.1"),
        ("allow-inventory", "0.2.0-rc.1"),
        ("allow-files", "0.2.0-rc.1"),
        ("allow-rust", "0.2.0-rc.1"),
        ("allow-match", "0.2.0-rc.1"),
        ("allow-report", "0.2.0-rc.1"),
        ("allow-diff", "0.2.0-rc.1"),
        ("allow-policy-legacy", "0.2.0-rc.1"),
        ("effortless-repo-protocol", "0.1.0"),
        ("effortless-repo-edit", "0.1.0"),
        ("effortless-repo-snapshot", "0.1.0"),
    ]
    .map(|(name, version)| IsolatedInstallPackageRowV2 {
        package_name: name.to_string(),
        package_version: version.to_string(),
        crate_digest: format!("sha256:{:064x}", name.len()),
        index_checksum: format!("sha256:{:064x}", name.len() + 1),
        resolved_version: Some(version.to_string()),
    });
    IsolatedInstallPayloadV2 {
        schema_id: "cargo-allow.isolated-install.v2".to_string(),
        schema_version: 2,
        candidate_artifact_digest: format!("sha256:{:064x}", 1),
        repository_commit: "0d63c071".to_string(),
        repository_tree: "tree-identity".to_string(),
        cargo_lock_digest: format!("sha256:{:064x}", 2),
        registry_index_digest: format!("sha256:{:064x}", 3),
        external_cache_identity: format!("sha256:{:064x}", 4),
        source_checkout_denied: true,
        install_root_identity: format!("sha256:{:064x}", 5),
        cargo_home_identity: format!("sha256:{:064x}", 6),
        installed_executable_digest: format!("sha256:{:064x}", 7),
        installed_version_output: "cargo-allow 0.2.0-rc.1".to_string(),
        platform: "x86_64-unknown-linux-gnu".to_string(),
        toolchain: "stable".to_string(),
        package_rows: rows.to_vec(),
        graph_comparison: IsolatedInstallGraphComparisonV2 {
            expected_packages: 13,
            matched_packages: 13,
            unexpected_packages: vec![],
            missing_packages: vec![],
            version_mismatches: vec![],
            path_sources: vec![],
        },
        limitations: vec!["linux hosted claim only".to_string()],
        claim_boundary: "isolated install evidence only".to_string(),
    }
}
