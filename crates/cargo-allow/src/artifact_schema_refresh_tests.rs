use crate::artifact_schema_support::{
    assert_required_fields, assert_schema_type_equals, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn refresh_schema_locks_drift_refresh_receipt_contract() {
    let schema = parse_schema(
        "refresh",
        include_str!("../../../docs/schemas/refresh.schema.json"),
    );

    assert_required_fields(
        "refresh",
        &schema,
        &[
            "mode",
            "summary",
            "previous_last_seen",
            "refreshed_last_seen",
            "matched_finding",
        ],
    );

    let summary = required_schema_pointer("refresh", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "refresh summary should reject unknown fields"
    );
    assert_required_fields(
        "refresh summary",
        summary,
        &["entry_id", "drift_message", "lifecycle_preserved"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/lifecycle_preserved/const")
            .and_then(Value::as_bool),
        Some(true),
        "refresh summary lifecycle_preserved should be const true"
    );

    let mode = required_schema_pointer("refresh", &schema, "/properties/mode");
    assert_required_fields(
        "refresh mode",
        mode,
        &[
            "dry_run",
            "write_requested",
            "explicit_dry_run",
            "written_path",
        ],
    );

    assert_schema_type_equals(
        "refresh previous_last_seen",
        &schema,
        "/properties/previous_last_seen/type",
        &["object", "null"],
    );
    assert_schema_type_equals(
        "refresh refreshed_last_seen",
        &schema,
        "/properties/refreshed_last_seen/type",
        &["object", "null"],
    );
}
