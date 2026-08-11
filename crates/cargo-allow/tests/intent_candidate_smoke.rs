//! Offline characterization for IntentCandidateInstallSmokeV1 (#2599-C).
//!
//! The package/extract/install harness lives in `scripts/intent-candidate-smoke.sh`
//! so release automation can invoke Cargo without violating the product source-tree
//! invariant.

const SCHEMA_ID: &str = "cargo-allow.intent-candidate-smoke.v1";
const CRATE_SET_SCHEMA_ID: &str = "cargo-allow.intent-candidate-crate-set.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/intent-candidate-smoke-pass.example.json");
const CRATE_SET: &str =
    include_str!("../../../docs/dogfood/fixtures/release/intent-candidate-crate-set.toml");

const EXPECTED_CRATES: &[&str] = &[
    "effortless-repo-protocol",
    "effortless-repo-snapshot",
    "effortless-rust-source-index",
    "intent-model",
    "intent-protocol",
    "intent-compiler",
    "cargo-intent",
];

#[test]
fn example_intent_candidate_smoke_matches_schema_constants() {
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
    let boundary = example
        .get("claim_boundary")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary missing"));
    for required in [
        "seven_crate_intent_package_graph",
        "no_proof_or_test_invocation",
        "no_workspace_target_debug_binary",
        "source_checkout_denied_during_decisive_install",
    ] {
        assert!(
            boundary.iter().any(|v| v.as_str() == Some(required)),
            "example claim_boundary missing {required}"
        );
    }
    assert_eq!(
        example
            .pointer("/environment/isolation_mechanism")
            .and_then(serde_json::Value::as_str),
        Some("path_patch_extracted")
    );
    assert_eq!(
        example
            .pointer("/install/method")
            .and_then(serde_json::Value::as_str),
        Some("cargo_install_path_extracted_with_patch")
    );
}

#[test]
fn intent_candidate_crate_set_fixture_lists_seven_package_order_crates() {
    for name in EXPECTED_CRATES {
        assert!(
            CRATE_SET.contains(&format!("\"{name}\"")),
            "intent-candidate-crate-set.toml missing {name}"
        );
    }
}
