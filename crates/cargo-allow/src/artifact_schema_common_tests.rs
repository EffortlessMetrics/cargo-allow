use crate::artifact_schema_support::{
    assert_command_contract, assert_enum_contains_all, assert_inventory_schema,
    assert_required_fields, parse_schema, required_schema_pointer, schema_contracts,
};
use serde_json::Value;

#[test]
fn schema_files_require_common_v1_source_tree_contract() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);

        assert_eq!(
            schema
                .pointer("/properties/schema_version/const")
                .and_then(Value::as_u64),
            Some(u64::from(contract.schema_version)),
            "{} schema_version const",
            contract.name
        );
        assert_eq!(
            schema
                .pointer("/properties/schema_id/const")
                .and_then(Value::as_str),
            Some(contract.schema_id),
            "{} schema_id const",
            contract.name
        );
        assert_required_fields(
            contract.name,
            &schema,
            &[
                "schema_version",
                "schema_id",
                "tool",
                "command",
                "claim_boundary",
                "scanner_limitations",
                "inventory",
            ],
        );
        assert_eq!(
            schema
                .pointer("/properties/tool/const")
                .and_then(Value::as_str),
            Some("cargo-allow"),
            "{} tool const",
            contract.name
        );
        assert_command_contract(contract, &schema);
        assert_inventory_schema(contract.name, &schema);
        assert_enum_contains_all(
            contract.name,
            &schema,
            "/$defs/claim_boundary_flag/enum",
            allow_report::CLAIM_BOUNDARY,
        );
        assert_enum_contains_all(
            contract.name,
            &schema,
            "/$defs/scanner_limitation/enum",
            allow_report::SCANNER_LIMITATIONS,
        );
    }
}

#[test]
fn report_schema_locks_diff_posture_extension_contract() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/diff/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/diff"),
        "report diff property should reference the diff extension schema"
    );

    let diff = required_schema_pointer("report", &schema, "/$defs/diff");
    assert_eq!(
        diff.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "report diff should reject unknown fields"
    );
    assert_required_fields(
        "report diff",
        diff,
        &[
            "net_posture",
            "reviewer_action",
            "summary",
            "finding_changes",
            "policy_changes",
        ],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/diff/properties/net_posture/enum",
        &["worse", "review-required", "improved", "unchanged"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/summary/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/diff_summary"),
        "report diff summary should reference the diff summary schema"
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/finding_changes/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/finding_posture_change"),
        "report diff finding_changes should use finding posture rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/policy_changes/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/policy_change"),
        "report diff policy_changes should use policy change rows"
    );

    assert_required_fields(
        "report diff summary",
        required_schema_pointer("report", &schema, "/$defs/diff_summary"),
        &[
            "current_failures",
            "new_findings",
            "removed_findings",
            "policy_failures",
            "policy_review_items",
            "policy_improvements",
        ],
    );
    assert_required_fields(
        "report finding posture change",
        required_schema_pointer("report", &schema, "/$defs/finding_posture_change"),
        &["change", "key", "kind", "family", "path"],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/finding_posture_change/properties/change/enum",
        &["new", "removed"],
    );
    assert_required_fields(
        "report policy change",
        required_schema_pointer("report", &schema, "/$defs/policy_change"),
        &["severity", "allow_id", "kind", "message"],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/policy_change/properties/severity/enum",
        &["improvement", "review", "fail"],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/policy_change/properties/kind/enum",
        &[
            "added_allow",
            "removed_allow",
            "baseline_debt_added",
            "scope_broadened",
            "scope_narrowed",
            "selector_precision_decreased",
            "selector_precision_increased",
            "expiry_extended",
            "expiry_shortened",
            "review_after_extended",
            "review_after_shortened",
            "evidence_added",
            "evidence_removed",
            "owner_added",
            "owner_removed",
            "reason_added",
            "reason_removed",
            "classification_added",
            "classification_removed",
            "occurrence_limit_tightened",
            "occurrence_limit_loosened",
        ],
    );
}

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
    assert_enum_contains_all(
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
        "entries_with_evidence",
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
