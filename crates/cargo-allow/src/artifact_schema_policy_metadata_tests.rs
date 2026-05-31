use crate::artifact_schema_expectations::{
    exception_identity_change_fields, metadata_change_fields, requirement_change_fields,
};
use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;

#[test]
fn common_schema_policy_metadata_fragments_keep_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    assert_enum_equals(
        "common exception identity fields",
        &schema,
        "/$defs/exception_identity_change_field/enum",
        &exception_identity_change_fields(),
    );
    let exception_identity_change =
        required_schema_pointer("common", &schema, "/$defs/exception_identity_change");
    assert_eq!(
        exception_identity_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common exception_identity_change should reject unknown fields"
    );
    assert_required_fields(
        "common exception_identity_change",
        exception_identity_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/exception_identity_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/exception_identity_change_field"),
        "common exception_identity_change field should use the shared exception identity field vocabulary"
    );
    assert_schema_type_equals(
        "common exception_identity_change before",
        &schema,
        "/$defs/exception_identity_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common exception_identity_change after",
        &schema,
        "/$defs/exception_identity_change/properties/after/type",
        &["string", "null"],
    );

    assert_enum_equals(
        "common metadata fields",
        &schema,
        "/$defs/metadata_change_field/enum",
        &metadata_change_fields(),
    );
    let metadata_change = required_schema_pointer("common", &schema, "/$defs/metadata_change");
    assert_eq!(
        metadata_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common metadata_change should reject unknown fields"
    );
    assert_required_fields(
        "common metadata_change",
        metadata_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/metadata_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/metadata_change_field"),
        "common metadata_change field should use the shared metadata field vocabulary"
    );
    assert_schema_type_equals(
        "common metadata_change before",
        &schema,
        "/$defs/metadata_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common metadata_change after",
        &schema,
        "/$defs/metadata_change/properties/after/type",
        &["string", "null"],
    );

    assert_enum_equals(
        "common requirement fields",
        &schema,
        "/$defs/requirement_change_field/enum",
        &requirement_change_fields(),
    );
    let requirement_change =
        required_schema_pointer("common", &schema, "/$defs/requirement_change");
    assert_eq!(
        requirement_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common requirement_change should reject unknown fields"
    );
    assert_required_fields(
        "common requirement_change",
        requirement_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/requirement_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/requirement_change_field"),
        "common requirement_change field should use the shared requirement field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/requirement_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("boolean"),
            "common requirement_change {field} type"
        );
    }

    let policy_status_change =
        required_schema_pointer("common", &schema, "/$defs/policy_status_change");
    assert_eq!(
        policy_status_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common policy_status_change should reject unknown fields"
    );
    assert_required_fields(
        "common policy_status_change",
        policy_status_change,
        &["before", "after"],
    );
    assert_schema_type_equals(
        "common policy_status_change before",
        &schema,
        "/$defs/policy_status_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common policy_status_change after",
        &schema,
        "/$defs/policy_status_change/properties/after/type",
        &["string", "null"],
    );
}
