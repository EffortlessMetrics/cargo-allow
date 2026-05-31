use crate::artifact_schema_expectations::{
    lifecycle_change_fields, scope_change_fields, selector_identity_change_fields,
    selector_precision_fields,
};
use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;

#[test]
fn common_schema_selector_lifecycle_fragments_keep_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    assert_enum_equals(
        "common selector precision fields",
        &schema,
        "/$defs/selector_precision_field/enum",
        &selector_precision_fields(),
    );
    assert_enum_equals(
        "common selector identity fields",
        &schema,
        "/$defs/selector_identity_change_field/enum",
        &selector_identity_change_fields(),
    );
    let selector_identity =
        required_schema_pointer("common", &schema, "/$defs/selector_identity_change");
    assert_eq!(
        selector_identity
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common selector_identity_change should reject unknown fields"
    );
    assert_required_fields(
        "common selector_identity_change",
        selector_identity,
        &["changed_fields"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector_identity_change/properties/changed_fields/type")
            .and_then(Value::as_str),
        Some("array"),
        "common selector identity changed_fields type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector_identity_change/properties/changed_fields/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/selector_identity_change_field"),
        "common selector identity changed_fields should use the selector identity field vocabulary"
    );

    let selector_precision =
        required_schema_pointer("common", &schema, "/$defs/selector_precision_change");
    assert_eq!(
        selector_precision
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common selector_precision_change should reject unknown fields"
    );
    assert_required_fields(
        "common selector_precision_change",
        selector_precision,
        &["before", "after", "removed_fields", "added_fields"],
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("integer"),
            "common selector precision {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "common selector precision {field} minimum"
        );
    }
    for field in ["removed_fields", "added_fields"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/items/$ref"
                ))
                .and_then(Value::as_str),
            Some("#/$defs/selector_precision_field"),
            "common selector precision {field} should use the field vocabulary"
        );
    }

    assert_enum_equals(
        "common lifecycle fields",
        &schema,
        "/$defs/lifecycle_change_field/enum",
        &lifecycle_change_fields(),
    );
    let lifecycle_change = required_schema_pointer("common", &schema, "/$defs/lifecycle_change");
    assert_eq!(
        lifecycle_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common lifecycle_change should reject unknown fields"
    );
    assert_required_fields(
        "common lifecycle_change",
        lifecycle_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/lifecycle_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/lifecycle_change_field"),
        "common lifecycle_change field should use the shared lifecycle field vocabulary"
    );
    assert_schema_type_equals(
        "common lifecycle_change before",
        &schema,
        "/$defs/lifecycle_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common lifecycle_change after",
        &schema,
        "/$defs/lifecycle_change/properties/after/type",
        &["string", "null"],
    );

    let occurrence_limit =
        required_schema_pointer("common", &schema, "/$defs/occurrence_limit_change");
    assert_eq!(
        occurrence_limit
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common occurrence_limit_change should reject unknown fields"
    );
    assert_required_fields(
        "common occurrence_limit_change",
        occurrence_limit,
        &["before", "after"],
    );
    for field in ["before", "after"] {
        assert_schema_type_equals(
            &format!("common occurrence_limit_change {field}"),
            &schema,
            &format!("/$defs/occurrence_limit_change/properties/{field}/type"),
            &["integer", "null"],
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/occurrence_limit_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "common occurrence_limit_change {field} minimum"
        );
    }

    assert_enum_equals(
        "common scope fields",
        &schema,
        "/$defs/scope_change_field/enum",
        &scope_change_fields(),
    );
    let scope_change = required_schema_pointer("common", &schema, "/$defs/scope_change");
    assert_eq!(
        scope_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common scope_change should reject unknown fields"
    );
    assert_required_fields(
        "common scope_change",
        scope_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/scope_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/scope_change_field"),
        "common scope_change field should use the shared scope field vocabulary"
    );
    assert_schema_type_equals(
        "common scope_change before",
        &schema,
        "/$defs/scope_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common scope_change after",
        &schema,
        "/$defs/scope_change/properties/after/type",
        &["string", "null"],
    );
}
