use crate::artifact_schema_support::{
    assert_required_fields, assert_schema_type_contains, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn add_schema_locks_selected_finding_and_review_contract() {
    let schema = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));

    assert_required_fields(
        "add",
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
            "allow_entry",
            "selected_finding",
        ],
    );

    let options = required_schema_pointer("add", &schema, "/properties/options");
    assert_eq!(
        options.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "add options should reject unknown fields"
    );
    assert_required_fields("add options", options, &["policy_output", "force"]);
    assert_schema_type_contains(
        "add options policy_output",
        &schema,
        "/properties/options/properties/policy_output/type",
        "string",
    );
    assert_schema_type_contains(
        "add options policy_output",
        &schema,
        "/properties/options/properties/policy_output/type",
        "null",
    );
    assert_eq!(
        schema
            .pointer("/properties/options/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "add force should be boolean"
    );

    let summary = required_schema_pointer("add", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "add summary should reject unknown fields"
    );
    assert_required_fields(
        "add summary",
        summary,
        &["entry_id", "selected_finding", "human_review_required"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/human_review_required/const")
            .and_then(Value::as_bool),
        Some(true),
        "add summaries should always require human review"
    );

    let allow_entry = required_schema_pointer("add", &schema, "/properties/allow_entry");
    assert_eq!(
        allow_entry
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add allow_entry should reject unknown fields"
    );
    assert_required_fields(
        "add allow_entry",
        allow_entry,
        &[
            "id",
            "kind",
            "family",
            "path",
            "glob",
            "owner",
            "classification",
            "reason",
            "review_after",
            "expires",
            "evidence_count",
            "selector",
            "last_seen",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/allow_entry/properties/evidence_count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "add allow_entry evidence_count should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/selected_finding/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/finding"),
        "add selected_finding should use finding rows"
    );
    let selected_finding = required_schema_pointer("add", &schema, "/$defs/finding");
    assert_eq!(
        selected_finding
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add selected findings should reject unknown fields"
    );
    assert_required_fields(
        "add selected finding",
        selected_finding,
        &[
            "status",
            "kind",
            "family",
            "path",
            "line",
            "column",
            "source_package",
            "identity",
            "message",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/status/const")
            .and_then(Value::as_str),
        Some("selected"),
        "add selected finding status should stay selected"
    );
    assert_schema_type_contains(
        "add selected finding source_package",
        &schema,
        "/$defs/finding/properties/source_package/type",
        "string",
    );
    assert_schema_type_contains(
        "add selected finding source_package",
        &schema,
        "/$defs/finding/properties/source_package/type",
        "null",
    );
}
