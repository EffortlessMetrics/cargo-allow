//! Offline characterization for ExtractionReadinessReceiptV1 (#2559).

const SCHEMA_ID: &str = "cargo-allow.extraction-readiness.v1";
const CHECKLIST_SCHEMA_ID: &str = "cargo-allow.extraction-readiness-checklist.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/extraction-readiness-pass.example.json");
const CHECKLIST_FIXTURE: &str =
    include_str!("../../../tests/fixtures/extraction-readiness/checklist-v1.toml");

const EXPECTED_CHECKLIST_ITEMS: &[&str] = &[
    "independent_packaging",
    "public_boundaries",
    "distinct_version_support_postures",
    "migration_shims_status",
    "forbidden_deps_absent",
    "dogfood_complete",
    "simplification_complete",
    "rollback_documented",
    "no_physical_extraction",
];

#[test]
fn example_extraction_readiness_matches_schema_constants() {
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| panic!("example receipt json: {err}"));
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
            .pointer("/checklist/schema_id")
            .and_then(serde_json::Value::as_str),
        Some(CHECKLIST_SCHEMA_ID)
    );
    let boundary = example
        .get("claim_boundary")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary missing"));
    for required in [
        "no_physical_repository_extraction",
        "separate_authorization_still_required",
    ] {
        assert!(
            boundary.iter().any(|v| v.as_str() == Some(required)),
            "example claim_boundary missing {required}"
        );
    }
    let prereqs = example
        .get("prerequisite_receipts")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| std::panic::panic_any("prerequisite_receipts missing"));
    for key in ["dogfood", "simplification"] {
        assert!(
            prereqs.contains_key(key),
            "example prerequisite_receipts missing {key}"
        );
    }
}

#[test]
fn extraction_readiness_checklist_lists_nine_gate_items() {
    for item_id in EXPECTED_CHECKLIST_ITEMS {
        assert!(
            CHECKLIST_FIXTURE.contains(&format!("id = \"{item_id}\"")),
            "checklist fixture missing {item_id}"
        );
    }
    assert!(
        CHECKLIST_FIXTURE.contains("no_physical_extraction"),
        "checklist must forbid physical extraction in this packet"
    );
}
