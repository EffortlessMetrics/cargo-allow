use crate::artifact_schema_expectations::structural_identity_fields;
use crate::artifact_schema_support::{
    assert_required_fields, assert_schema_type_equals, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn common_schema_identity_fragments_keep_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    let structural_identity =
        required_schema_pointer("common", &schema, "/$defs/structural_identity");
    assert_eq!(
        structural_identity
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common structural_identity should reject unknown fields"
    );
    assert_required_fields(
        "common structural_identity",
        structural_identity,
        &structural_identity_fields(),
    );
    for field in ["language", "ast_kind"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/structural_identity/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "common structural_identity {field} type"
        );
    }
    for field in [
        "crate_name",
        "module",
        "container",
        "symbol",
        "callee",
        "macro_name",
        "lint",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
    ] {
        assert_schema_type_equals(
            &format!("common structural_identity {field}"),
            &schema,
            &format!("/$defs/structural_identity/properties/{field}/type"),
            &["string", "null"],
        );
    }
    for field in ["line_hint", "column_hint"] {
        assert_schema_type_equals(
            &format!("common structural_identity {field}"),
            &schema,
            &format!("/$defs/structural_identity/properties/{field}/type"),
            &["integer", "null"],
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/structural_identity/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common structural_identity {field} minimum"
        );
    }

    let selector = required_schema_pointer("common", &schema, "/$defs/selector");
    assert_eq!(
        selector
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common selector should reject unknown fields"
    );
    assert_required_fields(
        "common selector",
        selector,
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
            "line_hint",
            "glob",
        ],
    );
    for field in [
        "ast_kind",
        "container",
        "callee",
        "macro_name",
        "lint",
        "symbol",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
        "glob",
    ] {
        assert_schema_type_equals(
            &format!("common selector {field}"),
            &schema,
            &format!("/$defs/selector/properties/{field}/type"),
            &["string", "null"],
        );
    }
    assert_schema_type_equals(
        "common selector line_hint",
        &schema,
        "/$defs/selector/properties/line_hint/type",
        &["integer", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector/properties/line_hint/minimum")
            .and_then(Value::as_u64),
        Some(1),
        "common selector line_hint minimum"
    );
}
