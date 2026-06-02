use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn migrate_schema_locks_policy_migration_summary_contract() {
    let schema = parse_schema(
        "migrate",
        include_str!("../../../docs/schemas/migrate.schema.json"),
    );

    assert_required_fields(
        "migrate",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "input",
            "output",
            "summary",
            "notes",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/scanner/const")
            .and_then(Value::as_str),
        Some("policy_migration"),
        "migrate inventory scanner should stay policy_migration"
    );

    let input = required_schema_pointer("migrate", &schema, "/properties/input");
    assert_eq!(
        input.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate input should reject unknown fields"
    );
    assert_required_fields("migrate input", input, &["kind", "path"]);
    assert_enum_equals(
        "migrate",
        &schema,
        "/properties/input/properties/kind/enum",
        &["from", "repo_policy"],
    );
    assert_eq!(
        schema
            .pointer("/properties/input/properties/path/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate input path should be a string"
    );

    let output = required_schema_pointer("migrate", &schema, "/properties/output");
    assert_eq!(
        output.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate output should reject unknown fields"
    );
    assert_required_fields("migrate output", output, &["path", "force"]);
    assert_eq!(
        schema
            .pointer("/properties/output/properties/path/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate output path should be a string"
    );
    assert_eq!(
        schema
            .pointer("/properties/output/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "migrate output force should be boolean"
    );

    let summary = required_schema_pointer("migrate", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate summary should reject unknown fields"
    );
    assert_required_fields(
        "migrate summary",
        summary,
        &[
            "allow_entries",
            "baseline_debt",
            "unsafe_entries",
            "entries_with_evidence",
        ],
    );
    for field in [
        "allow_entries",
        "baseline_debt",
        "unsafe_entries",
        "lint_exception_entries",
        "entries_with_evidence",
        "broken_evidence_links",
        "unsafe_broken_evidence_links",
        "weak_evidence_references",
        "unsafe_weak_evidence_references",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/properties/summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "migrate summary {field} should be an integer"
        );
    }
    assert_eq!(
        schema
            .pointer("/properties/notes/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate notes should be a string"
    );
}
