use crate::artifact_schema_support::{assert_enum_equals, parse_schema, required_schema_pointer};
use serde_json::Value;

#[test]
fn receipt_schema_locks_top_level_status_vocabulary() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    assert_enum_equals(
        "receipt status",
        &schema,
        "/properties/status/enum",
        allow_report::ARTIFACT_STATUSES,
    );
}

#[test]
fn receipt_schema_allows_optional_policy_baseline_debt_count() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    let count = required_schema_pointer(
        "receipt",
        &schema,
        "/$defs/counts/properties/policy_baseline_debt",
    );
    assert_eq!(
        count.get("type").and_then(Value::as_str),
        Some("integer"),
        "receipt policy_baseline_debt count type"
    );
    assert_eq!(
        count.get("minimum").and_then(Value::as_u64),
        Some(0),
        "receipt policy_baseline_debt count minimum"
    );
}

#[test]
fn receipt_schema_allows_optional_broken_evidence_link_count() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    let count = required_schema_pointer(
        "receipt",
        &schema,
        "/$defs/counts/properties/broken_evidence_links",
    );
    assert_eq!(
        count.get("type").and_then(Value::as_str),
        Some("integer"),
        "receipt broken_evidence_links count type"
    );
    assert_eq!(
        count.get("minimum").and_then(Value::as_u64),
        Some(0),
        "receipt broken_evidence_links count minimum"
    );
}
