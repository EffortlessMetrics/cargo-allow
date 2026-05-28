use crate::artifact_schema_support::{
    assert_enum_contains_all, assert_required_fields, assert_schema_type_contains, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;

#[test]
fn doctor_schema_locks_setup_artifact_contract() {
    let schema = parse_schema(
        "doctor",
        include_str!("../../../docs/schemas/doctor.schema.json"),
    );

    assert_required_fields(
        "doctor",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "root",
            "config",
            "inventory",
        ],
    );
    let root = required_schema_pointer("doctor", &schema, "/properties/root");
    assert_eq!(
        root.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor root should reject unknown fields"
    );
    assert_required_fields("doctor root", root, &["path", "discovery"]);
    assert_enum_contains_all(
        "doctor",
        &schema,
        "/properties/root/properties/discovery/enum",
        &[
            "explicit_root",
            "nearest_git_root",
            "current_directory_fallback",
        ],
    );

    let config = required_schema_pointer("doctor", &schema, "/properties/config");
    assert_eq!(
        config.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor config should reject unknown fields"
    );
    assert_required_fields("doctor config", config, &["found", "path"]);
    assert_eq!(
        schema
            .pointer("/properties/config/properties/found/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "doctor config found should be boolean"
    );
    assert_schema_type_contains(
        "doctor config path",
        &schema,
        "/properties/config/properties/path/type",
        "string",
    );
    assert_schema_type_contains(
        "doctor config path",
        &schema,
        "/properties/config/properties/path/type",
        "null",
    );

    assert_eq!(
        schema
            .pointer("/properties/inventory/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/inventory"),
        "doctor inventory should use the inventory schema"
    );
    assert_required_fields(
        "doctor inventory",
        required_schema_pointer("doctor", &schema, "/$defs/inventory"),
        &["scope", "scanner", "source", "files_scanned"],
    );
}
