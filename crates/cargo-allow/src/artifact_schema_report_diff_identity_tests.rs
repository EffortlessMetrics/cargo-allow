use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, enum_strings, parse_schema, required_schema_pointer,
};
use allow_diff::ExceptionIdentityChangeField;
use serde_json::Value;

#[test]
fn report_schema_locks_diff_identity_and_precision_contracts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let exception_identity_change =
        required_schema_pointer("report", &schema, "/$defs/exception_identity_change");
    assert_eq!(
        exception_identity_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report exception identity changes should reject unknown fields"
    );
    assert_required_fields(
        "report exception identity change",
        exception_identity_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/exception_identity_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/exception_identity_change_field"),
        "report exception identity changes should use the exception identity field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/exception_identity_change/properties/{field}/type/0"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "report exception identity {field} first type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/exception_identity_change/properties/{field}/type/1"
                ))
                .and_then(Value::as_str),
            Some("null"),
            "report exception identity {field} second type"
        );
    }
    assert_enum_equals(
        "report exception identity fields",
        &schema,
        "/$defs/exception_identity_change_field/enum",
        &enum_strings(
            ExceptionIdentityChangeField::ALL,
            ExceptionIdentityChangeField::as_str,
        ),
    );

    let selector_identity_change =
        required_schema_pointer("report", &schema, "/$defs/selector_identity_change");
    assert_eq!(
        selector_identity_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report selector identity changes should reject unknown fields"
    );
    assert_required_fields(
        "report selector identity change",
        selector_identity_change,
        &["changed_fields"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector_identity_change/properties/changed_fields/type")
            .and_then(Value::as_str),
        Some("array"),
        "report selector identity changed_fields type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector_identity_change/properties/changed_fields/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/selector_identity_change_field"),
        "report selector identity changed_fields should use the selector identity field vocabulary"
    );
    assert_enum_equals(
        "report selector identity fields",
        &schema,
        "/$defs/selector_identity_change_field/enum",
        &[
            "ast_kind",
            "container",
            "callee",
            "macro_name",
            "lint",
            "symbol",
            "receiver_fingerprint",
            "target_fingerprint",
            "normalized_snippet_hash",
        ],
    );

    let selector_precision =
        required_schema_pointer("report", &schema, "/$defs/selector_precision_change");
    assert_eq!(
        selector_precision
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report selector precision changes should reject unknown fields"
    );
    assert_required_fields(
        "report selector precision change",
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
            "report selector precision {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "report selector precision {field} minimum"
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
            "report selector precision {field} should use the field vocabulary"
        );
    }
    assert_enum_equals(
        "report selector precision fields",
        &schema,
        "/$defs/selector_precision_field/enum",
        &[
            "path",
            "glob",
            "family",
            "ast_kind",
            "container",
            "callee",
            "macro_name",
            "lint",
            "symbol",
            "receiver_fingerprint",
            "target_fingerprint",
            "normalized_snippet_hash",
            "occurrence_limit",
        ],
    );
}
