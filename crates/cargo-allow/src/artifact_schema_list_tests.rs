use crate::artifact_schema_support::{
    GOVERNED_KIND_ENUM, assert_enum_equals, assert_required_fields, parse_schema,
    required_schema_pointer,
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
            "review_after",
            "expires",
            "reason",
        ],
    );
    assert_enum_equals(
        "list allow entry kind",
        &schema,
        "/$defs/allow_entry/properties/kind/enum",
        GOVERNED_KIND_ENUM,
    );
}
