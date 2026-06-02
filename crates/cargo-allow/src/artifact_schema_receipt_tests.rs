use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, parse_schema, required_schema_pointer,
};
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
fn receipt_schema_locks_check_command_producer() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/command/const")
            .and_then(Value::as_str),
        Some("check"),
        "receipt command producer"
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

#[test]
fn receipt_schema_allows_optional_weak_evidence_reference_count() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    let count = required_schema_pointer(
        "receipt",
        &schema,
        "/$defs/counts/properties/weak_evidence_references",
    );
    assert_eq!(
        count.get("type").and_then(Value::as_str),
        Some("integer"),
        "receipt weak_evidence_references count type"
    );
    assert_eq!(
        count.get("minimum").and_then(Value::as_u64),
        Some(0),
        "receipt weak_evidence_references count minimum"
    );
}

#[test]
fn receipt_schema_allows_optional_policy_missing_evidence_count() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    let count = required_schema_pointer(
        "receipt",
        &schema,
        "/$defs/counts/properties/policy_missing_evidence",
    );
    assert_eq!(
        count.get("type").and_then(Value::as_str),
        Some("integer"),
        "receipt policy_missing_evidence count type"
    );
    assert_eq!(
        count.get("minimum").and_then(Value::as_u64),
        Some(0),
        "receipt policy_missing_evidence count minimum"
    );
}

#[test]
fn receipt_schema_allows_optional_evidence_repair_queues() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "receipt evidence repair queues should be an optional array"
    );
    let queue = required_schema_pointer(
        "receipt",
        &schema,
        "/properties/evidence_repair_queues/items",
    );
    assert_eq!(
        queue.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "receipt evidence repair queue rows should reject unknown fields"
    );
    assert_required_fields(
        "receipt evidence repair queue",
        queue,
        &["signal", "count", "command"],
    );
    assert_enum_equals(
        "receipt evidence repair queue signal",
        &schema,
        "/properties/evidence_repair_queues/items/properties/signal/enum",
        &[
            "broken_evidence_links",
            "missing_evidence",
            "weak_evidence_references",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "receipt evidence repair queue count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "receipt evidence repair queue count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "receipt evidence repair queue command should be a string"
    );
}

#[test]
fn receipt_schema_allows_optional_source_inventory() {
    let schema = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );

    let source_inventory =
        required_schema_pointer("receipt", &schema, "/properties/source_inventory/$ref");
    assert_eq!(
        source_inventory.as_str(),
        Some("#/$defs/source_inventory"),
        "receipt source_inventory should use the shared source inventory shape"
    );
    for pointer in [
        "/$defs/source_inventory/properties/findings",
        "/$defs/source_inventory_kind_row/properties/total",
        "/$defs/source_inventory_kind_row/properties/matched",
        "/$defs/source_inventory_kind_row/properties/new",
        "/$defs/source_inventory_kind_row/properties/review_items",
        "/$defs/source_inventory_family_row/properties/total",
        "/$defs/source_inventory_family_row/properties/matched",
        "/$defs/source_inventory_family_row/properties/new",
        "/$defs/source_inventory_family_row/properties/review_items",
    ] {
        let value = required_schema_pointer("receipt", &schema, pointer);
        assert_eq!(
            value.get("type").and_then(Value::as_str),
            Some("integer"),
            "{pointer} should be an integer"
        );
        assert_eq!(
            value.get("minimum").and_then(Value::as_u64),
            Some(0),
            "{pointer} minimum"
        );
    }
}
