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
        &["check", "audit", "worklist", "doctor", "explain"],
    );

    let explained_artifact_id =
        required_schema_pointer("spec-system", &schema, "/properties/explained_artifact_id");
    assert_eq!(
        explained_artifact_id.get("type").and_then(Value::as_str),
        Some("string"),
        "spec-system explain artifact id should be optional string"
    );
    let proof_commands =
        required_schema_pointer("spec-system", &schema, "/properties/proof_commands");
    assert_eq!(
        proof_commands.get("type").and_then(Value::as_str),
        Some("array"),
        "spec-system explain proof commands should be optional array"
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

    let summary = required_schema_pointer("spec-system", &schema, "/$defs/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "spec-system summary should reject unknown fields"
    );
    assert_required_fields(
        "spec-system summary",
        summary,
        &[
            "artifacts",
            "links",
            "support_tier_rows",
            "findings",
            "work_items",
        ],
    );

    let finding = required_schema_pointer("spec-system", &schema, "/$defs/finding");
    assert_eq!(
        finding.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "spec-system findings should reject unknown fields"
    );
    assert_required_fields(
        "spec-system finding",
        finding,
        &["kind", "message", "blocking_eligible"],
    );
    assert_enum_equals(
        "spec-system finding kind",
        &schema,
        "/$defs/finding_kind/enum",
        &[
            "profile_config",
            "doc_artifact_ledger",
            "artifact_file",
            "artifact_link",
            "active_goal",
            "support_tier",
            "federation_config",
            "import_graph",
        ],
    );
    assert_enum_equals(
        "spec-system blocking reason",
        &schema,
        "/$defs/blocking_reason/enum",
        &[
            "profile_config_parse_failure",
            "federation_config_invalid",
            "dialect_conflict",
            "federation_config_parse_failure",
            "doc_artifact_ledger_missing",
            "doc_artifact_ledger_parse_failure",
            "duplicate_id",
            "invalid_artifact_kind_or_status",
            "artifact_file_missing",
            "artifact_file_unreadable",
            "artifact_id_not_in_file",
            "unknown_link_target",
        ],
    );

    let work_item = required_schema_pointer("spec-system", &schema, "/$defs/work_item");
    assert_eq!(
        work_item
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "spec-system work items should reject unknown fields"
    );
    assert_required_fields(
        "spec-system work item",
        work_item,
        &["kind", "message", "suggested_actions", "proof_commands"],
    );
    assert_enum_equals(
        "spec-system work item kind",
        &schema,
        "/$defs/work_item_kind/enum",
        &[
            "missing_node",
            "missing_doc_artifact",
            "artifact_file_missing",
            "artifact_file_unreadable",
            "artifact_id_not_in_file",
            "invalid_artifact_status",
            "missing_required_edge",
            "missing_linked_proposal",
            "unknown_link_target",
            "unknown_linked_artifact",
            "orphan_spec",
            "missing_support_tier",
            "missing_proof_command",
            "claim_without_support_tier",
            "stale_active_goal",
            "legacy_goal_historical_only",
            "missing_closeout",
            "superseded_target_missing",
            "broken_import",
            "missing_import_root",
        ],
    );
}
