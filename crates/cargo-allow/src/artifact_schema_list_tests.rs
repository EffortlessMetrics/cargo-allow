use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, governed_kind_enum, match_status_enum,
    parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn list_schema_locks_allow_entry_kind_contract() {
    let schema = parse_schema(
        "list",
        include_str!("../../../docs/schemas/list.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/allow_entries/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/allow_entry"),
        "list allow_entries should use allow entry rows"
    );

    let allow_entry = required_schema_pointer("list", &schema, "/$defs/allow_entry");
    assert_eq!(
        allow_entry
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "list allow entries should reject unknown fields"
    );
    assert_required_fields(
        "list allow entry",
        allow_entry,
        &[
            "id",
            "status",
            "matches",
            "kind",
            "family",
            "owner",
            "classification",
            "scope",
            "source_package",
            "evidence_count",
            "selector_precision",
            "broad_scope",
            "review_after",
            "expires",
            "reason",
        ],
    );
    let selector_precision = required_schema_pointer(
        "list allow entry",
        allow_entry,
        "/properties/selector_precision",
    );
    assert_eq!(
        selector_precision.get("type").and_then(Value::as_str),
        Some("integer"),
        "list selector_precision type"
    );
    assert_eq!(
        selector_precision.get("minimum").and_then(Value::as_u64),
        Some(0),
        "list selector_precision minimum"
    );
    assert_eq!(
        allow_entry
            .pointer("/properties/broad_scope/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "list broad_scope type"
    );
    assert_enum_equals(
        "list allow entry kind",
        &schema,
        "/$defs/allow_entry/properties/kind/enum",
        &governed_kind_enum(),
    );
    assert_enum_equals(
        "list allow entry status",
        &schema,
        "/$defs/allow_entry/properties/status/enum",
        &match_status_enum(),
    );
}
