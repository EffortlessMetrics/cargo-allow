use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn report_schema_allows_optional_policy_baseline_debt_summary_count() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let policy_baseline_debt = required_schema_pointer(
        "report",
        &schema,
        "/$defs/summary/properties/policy_baseline_debt",
    );
    assert_eq!(
        policy_baseline_debt.get("type").and_then(Value::as_str),
        Some("integer"),
        "report policy_baseline_debt count type"
    );
    assert_eq!(
        policy_baseline_debt.get("minimum").and_then(Value::as_u64),
        Some(0),
        "report policy_baseline_debt count minimum"
    );
}

#[test]
fn report_schema_locks_top_level_status_vocabulary() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_enum_equals(
        "report status",
        &schema,
        "/properties/status/enum",
        allow_report::ARTIFACT_STATUSES,
    );
}

#[test]
fn report_schema_locks_report_command_producers() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_enum_equals(
        "report command",
        &schema,
        "/properties/command/enum",
        allow_report::REPORT_COMMANDS,
    );
}

#[test]
fn report_schema_allows_optional_broken_evidence_link_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/broken_evidence_links",
        "/$defs/trend/properties/broken_evidence_links",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }
}

#[test]
fn report_schema_allows_optional_weak_evidence_reference_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/weak_evidence_references",
        "/$defs/trend/properties/weak_evidence_references",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }
}

#[test]
fn report_schema_allows_optional_evidence_repair_queues() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "report evidence repair queues should be an optional array"
    );
    let queue = required_schema_pointer(
        "report",
        &schema,
        "/properties/evidence_repair_queues/items",
    );
    assert_eq!(
        queue.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "report evidence repair queue rows should reject unknown fields"
    );
    assert_required_fields(
        "report evidence repair queue",
        queue,
        &["signal", "count", "command"],
    );
    assert_enum_equals(
        "report evidence repair queue signal",
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
        "report evidence repair queue count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "report evidence repair queue count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "report evidence repair queue command should be a string"
    );
}

#[test]
fn report_schema_allows_optional_policy_missing_evidence_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/policy_missing_evidence",
        "/$defs/trend/properties/policy_missing_evidence",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }
}

#[test]
fn report_schema_allows_optional_source_inventory() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let source_inventory =
        required_schema_pointer("report", &schema, "/properties/source_inventory/$ref");
    assert_eq!(
        source_inventory.as_str(),
        Some("#/$defs/source_inventory"),
        "report source_inventory should use the shared source inventory fragment"
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
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }

    for pointer in [
        "/$defs/source_inventory_kind_row/properties/kind/$ref",
        "/$defs/source_inventory_family_row/properties/kind/$ref",
    ] {
        assert_eq!(
            required_schema_pointer("report", &schema, pointer).as_str(),
            Some("#/$defs/governed_source_exception_kind"),
            "report {pointer} kind vocabulary"
        );
    }
}
