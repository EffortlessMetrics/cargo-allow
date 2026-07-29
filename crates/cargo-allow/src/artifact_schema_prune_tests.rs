use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, governed_kind_enum,
    parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn prune_schema_locks_stale_cleanup_artifact_contract() {
    let schema = parse_schema(
        "prune",
        include_str!("../../../docs/schemas/prune.schema.json"),
    );

    assert_required_fields(
        "prune",
        &schema,
        &["mode", "summary", "stale_entries", "mutation_receipt"],
    );

    let mode = required_schema_pointer("prune", &schema, "/properties/mode");
    assert_eq!(
        mode.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "prune mode should reject unknown fields"
    );
    assert_required_fields(
        "prune mode",
        mode,
        &[
            "dry_run",
            "write_requested",
            "explicit_dry_run",
            "written_path",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/mode/properties/dry_run/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "prune mode dry_run should be boolean"
    );
    assert_eq!(
        schema
            .pointer("/properties/mode/properties/write_requested/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "prune mode write_requested should be boolean"
    );
    assert_eq!(
        schema
            .pointer("/properties/mode/properties/explicit_dry_run/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "prune mode explicit_dry_run should be boolean"
    );
    assert_schema_type_equals(
        "prune mode written_path",
        &schema,
        "/properties/mode/properties/written_path/type",
        &["string", "null"],
    );

    let summary = required_schema_pointer("prune", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "prune summary should reject unknown fields"
    );
    assert_required_fields("prune summary", summary, &["stale_entries"]);
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/stale_entries/type")
            .and_then(Value::as_str),
        Some("integer"),
        "prune summary stale_entries should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/stale_entries/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/stale_entry"),
        "prune stale_entries should use stale entry rows"
    );
    let stale_entry = required_schema_pointer("prune", &schema, "/$defs/stale_entry");
    assert_eq!(
        stale_entry
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "prune stale entries should reject unknown fields"
    );
    assert_required_fields(
        "prune stale entry",
        stale_entry,
        &["id", "kind", "owner", "classification", "scope", "reason"],
    );
    assert_enum_equals(
        "prune stale entry kind",
        &schema,
        "/$defs/stale_entry/properties/kind/enum",
        &governed_kind_enum(),
    );
    let mutation_receipt = required_schema_pointer("prune", &schema, "/$defs/mutation_receipt");
    assert_eq!(
        mutation_receipt
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "prune mutation receipt should reject unknown fields"
    );
    assert_required_fields(
        "prune mutation receipt",
        mutation_receipt,
        &[
            "schema_id",
            "operation",
            "tool_version",
            "repo_root",
            "config_source",
            "ledger_ids",
            "changed_allow_ids",
            "before_fingerprints",
            "after_fingerprints",
            "result",
            "next_commands",
            "claim_boundary",
        ],
    );
    assert_eq!(
        mutation_receipt
            .pointer("/properties/operation/const")
            .and_then(Value::as_str),
        Some("prune"),
        "prune mutation receipt operation should be pinned"
    );
}
