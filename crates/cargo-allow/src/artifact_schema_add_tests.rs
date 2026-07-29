use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, governed_kind_enum,
    parse_schema, required_schema_pointer,
};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn add_schema_locks_selected_finding_and_review_contract() {
    let schema = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));

    assert_required_fields(
        "add",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "options",
            "summary",
            "allow_entry",
            "selected_finding",
        ],
    );

    let options = required_schema_pointer("add", &schema, "/properties/options");
    assert_eq!(
        options.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "add options should reject unknown fields"
    );
    assert!(
        options.get("required").is_none(),
        "add option fields should stay optional for add.v1 compatibility"
    );
    assert_add_option_properties(options);
    assert_schema_type_equals(
        "add options policy_output",
        &schema,
        "/properties/options/properties/policy_output/type",
        &["string", "null"],
    );
    assert_eq!(
        schema
            .pointer("/properties/options/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "add force should be boolean"
    );

    let summary = required_schema_pointer("add", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "add summary should reject unknown fields"
    );
    assert_required_fields(
        "add summary",
        summary,
        &["entry_id", "selected_finding", "human_review_required"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/human_review_required/const")
            .and_then(Value::as_bool),
        Some(true),
        "add summaries should always require human review"
    );

    let allow_entry = required_schema_pointer("add", &schema, "/properties/allow_entry");
    assert_eq!(
        allow_entry
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add allow_entry should reject unknown fields"
    );
    assert_required_fields(
        "add allow_entry",
        allow_entry,
        &[
            "id",
            "kind",
            "family",
            "path",
            "glob",
            "owner",
            "classification",
            "reason",
            "review_after",
            "expires",
            "evidence_count",
            "selector",
            "last_seen",
        ],
    );
    assert_enum_equals(
        "add allow_entry kind",
        &schema,
        "/properties/allow_entry/properties/kind/enum",
        &governed_kind_enum(),
    );
    assert_eq!(
        schema
            .pointer("/properties/allow_entry/properties/evidence_count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "add allow_entry evidence_count should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/selected_finding/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/finding"),
        "add selected_finding should use finding rows"
    );
    let selected_finding = required_schema_pointer("add", &schema, "/$defs/finding");
    assert_eq!(
        selected_finding
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add selected findings should reject unknown fields"
    );
    assert_required_fields(
        "add selected finding",
        selected_finding,
        &[
            "status", "kind", "path", "line", "column", "identity", "message",
        ],
    );
    assert_enum_equals(
        "add selected finding kind",
        &schema,
        "/$defs/finding/properties/kind/enum",
        &governed_kind_enum(),
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/status/const")
            .and_then(Value::as_str),
        Some("selected"),
        "add selected finding status should stay selected"
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/identity/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/structural_identity"),
        "add selected finding identity should use structural identity rows"
    );
    let structural_identity = required_schema_pointer("add", &schema, "/$defs/structural_identity");
    assert_eq!(
        structural_identity
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add structural identity should reject unknown fields"
    );
    assert_required_fields(
        "add structural identity",
        structural_identity,
        &[
            "language",
            "crate_name",
            "module",
            "container",
            "ast_kind",
            "symbol",
            "callee",
            "macro_name",
            "lint",
            "receiver_fingerprint",
            "target_fingerprint",
            "normalized_snippet_hash",
            "line_hint",
            "column_hint",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/source_package/type")
            .and_then(Value::as_str),
        Some("string"),
        "add selected finding source_package should be a string when present"
    );
}

#[test]
fn add_schema_locks_mutation_receipt_envelope() {
    let schema = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));

    let mutation_receipt = required_schema_pointer("add", &schema, "/$defs/mutation_receipt");
    assert_eq!(
        mutation_receipt
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "mutation_receipt should reject unknown fields"
    );
    assert_required_fields(
        "add mutation_receipt",
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
        schema
            .pointer("/$defs/mutation_receipt/properties/schema_id/const")
            .and_then(Value::as_str),
        Some("cargo-allow.mutation-receipt.v1"),
        "mutation_receipt schema_id should be pinned"
    );
    assert_schema_type_equals(
        "add mutation_receipt repo_root",
        &schema,
        "/$defs/mutation_receipt/properties/repo_root/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "add mutation_receipt before_fingerprints items",
        &schema,
        "/$defs/mutation_receipt/properties/before_fingerprints/items/type",
        &["string", "null"],
    );
}

fn assert_add_option_properties(options: &Value) {
    let Some(properties) = options.get("properties").and_then(Value::as_object) else {
        std::panic::panic_any("add options properties should be an object");
    };
    let actual = properties
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = ["policy_output", "force"]
        .into_iter()
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected, "add option schema properties");
}
