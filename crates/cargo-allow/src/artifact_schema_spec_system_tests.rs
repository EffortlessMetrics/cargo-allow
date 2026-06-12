use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn spec_system_schema_locks_profile_commands_and_readiness() {
    let schema = parse_schema(
        "spec-system",
        include_str!("../../../docs/schemas/spec-system.schema.json"),
    );

    assert_enum_equals(
        "spec-system command",
        &schema,
        "/properties/command/enum",
        &["check", "audit", "worklist", "doctor"],
    );

    let readiness = required_schema_pointer("spec-system", &schema, "/$defs/readiness");
    assert_eq!(
        readiness
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "spec-system readiness should reject unknown fields"
    );
    assert_required_fields(
        "spec-system readiness",
        readiness,
        &["ready", "mode", "checks"],
    );
    assert_enum_equals(
        "spec-system readiness mode",
        &schema,
        "/$defs/readiness/properties/mode/enum",
        &["advisory", "shadow", "blocking"],
    );

    let check = required_schema_pointer("spec-system", &schema, "/$defs/readiness_check");
    assert_eq!(
        check.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "spec-system readiness checks should reject unknown fields"
    );
    assert_required_fields(
        "spec-system readiness check",
        check,
        &["kind", "found", "valid", "status", "message"],
    );
    assert_enum_equals(
        "spec-system readiness check kind",
        &schema,
        "/$defs/readiness_check/properties/kind/enum",
        &[
            "profile_config",
            "artifact_root",
            "artifact_ledger",
            "support_tiers",
            "active_goal",
            "templates",
        ],
    );
    assert_enum_equals(
        "spec-system readiness check status",
        &schema,
        "/$defs/readiness_check/properties/status/enum",
        &["ready", "missing", "invalid"],
    );
}
