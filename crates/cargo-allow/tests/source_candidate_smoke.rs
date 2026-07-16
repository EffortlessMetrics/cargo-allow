//! Offline characterization for SourceCandidateSmokeReceiptV1 (#2278).
//!
//! The installed first-hour journey itself lives in
//! `scripts/source-candidate-smoke.sh` so release harnesses can invoke Cargo
//! without violating the product source-tree invariant that Rust sources must
//! not spawn Cargo/compiler tools.

const SCHEMA_ID: &str = "cargo-allow.source-candidate-smoke-receipt.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/source-candidate-smoke-pass.example.json");
const SCHEMA_DOC: &str = include_str!(
    "../../../docs/dogfood/fixtures/release/source-candidate-smoke-receipt.v1.schema.json"
);

const STEPS_EXPECTED: &[&str] = &[
    "version",
    "doctor_no_policy",
    "audit_with_finding",
    "bootstrap_propose_write",
    "check_no_new_pass",
    "list_explain_worklist",
];

#[test]
fn example_source_candidate_smoke_receipt_matches_schema_constants() {
    assert!(
        SCHEMA_DOC.contains(SCHEMA_ID),
        "schema fixture must pin {SCHEMA_ID}"
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
    let expected = example
        .pointer("/journey/steps_expected")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("steps_expected missing"));
    assert_eq!(expected.len(), STEPS_EXPECTED.len());
    for (idx, step) in STEPS_EXPECTED.iter().enumerate() {
        assert_eq!(
            expected.get(idx).and_then(serde_json::Value::as_str),
            Some(*step)
        );
    }
    let limitations = example
        .get("limitations")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("limitations missing"));
    assert!(
        limitations
            .iter()
            .any(|v| v.as_str() == Some("package_set_not_consumed_from_isolated_registry")),
        "example receipt must record #2277 isolation limitation"
    );
}
