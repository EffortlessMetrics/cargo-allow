//! Offline characterization for ExactCandidatePackageSetV1 (#2372 / #2277).
//!
//! The package/extract/vendor/install harness lives in
//! `scripts/exact-candidate-package-set.sh` so release automation can invoke
//! Cargo without violating the product source-tree invariant.

const SCHEMA_ID: &str = "cargo-allow.exact-candidate-package-set.v1";
const CRATE_SET_SCHEMA_ID: &str = "cargo-allow.candidate-crate-set.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/exact-candidate-package-set-pass.example.json");
const SCHEMA_DOC: &str = include_str!(
    "../../../docs/dogfood/fixtures/release/exact-candidate-package-set.v1.schema.json"
);
const CRATE_SET: &str =
    include_str!("../../../docs/dogfood/fixtures/release/candidate-crate-set.toml");

const EXPECTED_CRATES: &[&str] = &[
    "allow-core",
    "allow-policy",
    "allow-inventory",
    "allow-files",
    "allow-rust",
    "allow-match",
    "allow-report",
    "allow-policy-legacy",
    "allow-diff",
    "cargo-allow",
];

#[test]
fn example_exact_candidate_package_set_matches_schema_constants() {
    assert!(
        SCHEMA_DOC.contains(SCHEMA_ID),
        "schema fixture must pin {SCHEMA_ID}"
    );
    assert!(
        CRATE_SET.contains(CRATE_SET_SCHEMA_ID),
        "crate-set fixture must pin {CRATE_SET_SCHEMA_ID}"
    );
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt json: {err}")));
    assert_eq!(
        example.get("schema_id").and_then(serde_json::Value::as_str),
        Some(SCHEMA_ID)
    );
    assert_eq!(
        example.get("result").and_then(serde_json::Value::as_str),
        Some("Passed")
    );
    assert_eq!(
        example
            .pointer("/candidate/crate_set_schema_id")
            .and_then(serde_json::Value::as_str),
        Some(CRATE_SET_SCHEMA_ID)
    );
    let order = example
        .pointer("/package_set/order")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("order missing"));
    assert_eq!(order.len(), EXPECTED_CRATES.len());
    for (idx, name) in EXPECTED_CRATES.iter().enumerate() {
        assert_eq!(
            order.get(idx).and_then(serde_json::Value::as_str),
            Some(*name)
        );
    }
    let limitations = example
        .get("limitations")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("limitations missing"));
    assert!(
        limitations
            .iter()
            .any(|v| v.as_str() == Some("not_classic_transitive_local_registry_index")),
        "example must record classic local-registry follow-up limitation"
    );
    assert_eq!(
        example
            .pointer("/environment/isolation_mechanism")
            .and_then(serde_json::Value::as_str),
        Some("directory_source_replacement")
    );
    assert_eq!(
        example
            .pointer("/install/method")
            .and_then(serde_json::Value::as_str),
        Some("cargo_install_path_extracted_with_directory_source")
    );
    let negatives = example
        .get("negative_controls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("negative_controls missing"));
    let ids: Vec<&str> = negatives
        .iter()
        .filter_map(|v| v.get("id").and_then(serde_json::Value::as_str))
        .collect();
    for required in [
        "omit_internal_crate_from_patch",
        "workspace_path_install_rejected",
        "package_checksum_mutation_after_inventory",
        "injected_normalized_path_dependency",
        "older_internal_package_version",
        "omit_candidate_from_directory_vendor",
    ] {
        assert!(
            ids.contains(&required),
            "example receipt missing negative control {required}"
        );
    }
}

#[test]
fn candidate_crate_set_fixture_lists_ten_publish_order_crates() {
    for name in EXPECTED_CRATES {
        assert!(
            CRATE_SET.contains(&format!("\"{name}\"")),
            "candidate-crate-set.toml missing {name}"
        );
    }
}
