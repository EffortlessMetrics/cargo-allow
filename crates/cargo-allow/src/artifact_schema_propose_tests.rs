use crate::artifact_schema_support::{
    assert_required_fields, assert_schema_type_equals, parse_schema, required_schema_pointer,
};
use serde_json::Value;

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
    assert_required_fields(
        "propose options",
        options,
        &["kind", "expires", "policy_output", "force"],
    );
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
