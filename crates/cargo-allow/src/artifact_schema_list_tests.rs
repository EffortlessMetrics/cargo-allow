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

#[test]
fn list_schema_locks_allow_id_filter_contract() {
    let schema = parse_schema(
        "list",
        include_str!("../../../docs/schemas/list.schema.json"),
    );

    let filters = required_schema_pointer("list", &schema, "/properties/filters");
    assert_required_fields(
        "list filters",
        filters,
        &[
            "kind",
            "family",
            "owner",
            "classification",
            "path",
            "source_package",
            "status",
            "expired",
            "review_due",
            "stale",
            "baseline_debt",
            "broad_scope",
            "missing_evidence",
        ],
    );
    let required = filters
        .get("required")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("list filters required should be an array"));
    assert!(
        !required
            .iter()
            .any(|field| field.as_str() == Some("allow_id")),
        "allow_id filter should stay optional for list.v1 compatibility"
    );
    let allow_id = required_schema_pointer("list filters", filters, "/properties/allow_id");
    let types = allow_id
        .get("type")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("allow_id filter should be nullable string"));
    let types = types.iter().filter_map(Value::as_str).collect::<Vec<_>>();

    assert_eq!(types, vec!["string", "null"]);
}
