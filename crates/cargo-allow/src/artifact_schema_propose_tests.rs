use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn propose_schema_locks_generated_baseline_summary_contract() {
    let schema = parse_schema(
        "propose",
        include_str!("../../../docs/schemas/propose.schema.json"),
    );

    assert_required_fields(
        "propose",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "options",
            "summary",
            "generated_entry_defaults",
        ],
    );

    let options = required_schema_pointer("propose", &schema, "/properties/options");
    assert_eq!(
        options.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "propose options should reject unknown fields"
    );
    assert!(
        options.get("required").is_none(),
        "propose option fields should stay optional for propose.v1 compatibility"
    );
    assert_propose_option_properties(options);
    assert_schema_type_equals(
        "propose options kind",
        &schema,
        "/properties/options/properties/kind/type",
        &["string", "null"],
    );
    assert_eq!(
        schema
            .pointer("/properties/options/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "propose force should be boolean"
    );

    let summary = required_schema_pointer("propose", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "propose summary should reject unknown fields"
    );
    assert_required_fields(
        "propose summary",
        summary,
        &["findings_scanned", "baseline_debt_entries_proposed"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/findings_scanned/type")
            .and_then(Value::as_str),
        Some("integer"),
        "propose findings_scanned should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/baseline_debt_entries_proposed/type")
            .and_then(Value::as_str),
        Some("integer"),
        "propose baseline debt count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/unsafe_baseline_debt_entries_proposed/type")
            .and_then(Value::as_str),
        Some("integer"),
        "propose unsafe baseline debt count should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/follow_up_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "propose follow_up_queues should be an optional array"
    );
    assert_eq!(
        schema
            .pointer("/properties/follow_up_queues/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/follow_up_queue"),
        "propose follow_up_queues should use the queue row definition"
    );
    let queue = required_schema_pointer("propose", &schema, "/$defs/follow_up_queue");
    assert_eq!(
        queue.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "propose follow-up queue should reject unknown fields"
    );
    assert_required_fields(
        "propose follow-up queue",
        queue,
        &["signal", "route_kind", "item_kind", "count", "command"],
    );
    assert_enum_equals(
        "propose follow-up queue signal",
        &schema,
        "/$defs/follow_up_queue/properties/signal/enum",
        &[
            "baseline_debt_entries_proposed",
            "unsafe_baseline_debt_entries_proposed",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/label/type")
            .and_then(Value::as_str),
        Some("string"),
        "propose follow-up queue label should be a string"
    );
    assert_enum_equals(
        "propose follow-up queue route kind",
        &schema,
        "/$defs/follow_up_queue/properties/route_kind/enum",
        &["worklist_filter", "worklist_item_kind"],
    );
    assert_enum_equals(
        "propose follow-up queue item kind",
        &schema,
        "/$defs/follow_up_queue/properties/item_kind/enum",
        &["baseline_debt", "weak_evidence_reference"],
    );
    assert_enum_equals(
        "propose follow-up queue worklist filter",
        &schema,
        "/$defs/follow_up_queue/properties/worklist_filter/enum",
        &["baseline_debt"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "propose follow-up queue count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/count/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "propose follow-up queue count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "propose follow-up queue command should be a string"
    );

    let defaults =
        required_schema_pointer("propose", &schema, "/properties/generated_entry_defaults");
    assert_eq!(
        defaults
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "propose generated defaults should reject unknown fields"
    );
    assert_required_fields(
        "propose generated defaults",
        defaults,
        &["owner", "classification", "reason", "expires"],
    );
    assert_eq!(
        schema
            .pointer("/properties/generated_entry_defaults/properties/owner/const")
            .and_then(Value::as_str),
        Some("unowned"),
        "propose generated owner should stay visibly unowned"
    );
    assert_eq!(
        schema
            .pointer("/properties/generated_entry_defaults/properties/classification/const")
            .and_then(Value::as_str),
        Some("baseline_debt"),
        "propose generated classification should stay baseline_debt"
    );
}

fn assert_propose_option_properties(options: &Value) {
    let Some(properties) = options.get("properties").and_then(Value::as_object) else {
        std::panic::panic_any("propose options properties should be an object");
    };
    let actual = properties
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = ["kind", "expires", "policy_output", "force"]
        .into_iter()
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected, "propose option schema properties");
}
