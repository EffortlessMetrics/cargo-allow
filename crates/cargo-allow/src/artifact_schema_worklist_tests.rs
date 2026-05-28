use crate::artifact_schema_support::{
    assert_enum_contains_all, assert_required_fields, assert_schema_type_contains, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;

#[test]
fn worklist_schema_locks_filters_summary_and_work_items_contract() {
    let schema = parse_schema(
        "worklist",
        include_str!("../../../docs/schemas/worklist.schema.json"),
    );

    assert_required_fields(
        "worklist",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "summary",
            "work_items",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/filters/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/filters"),
        "worklist filters should use filters schema"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/summary"),
        "worklist summary should use summary schema"
    );
    assert_eq!(
        schema
            .pointer("/properties/work_items/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/work_item"),
        "worklist work_items should use work item rows"
    );

    let filters = required_schema_pointer("worklist", &schema, "/$defs/filters");
    assert_eq!(
        filters.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "worklist filters should reject unknown fields"
    );
    assert_required_fields("worklist filters", filters, &["kind", "risk", "difficulty"]);
    for field in [
        "kind",
        "family",
        "item_kind",
        "allow_id",
        "path",
        "source_package",
        "owner",
        "classification",
    ] {
        assert_schema_type_contains(
            "worklist filter string option",
            &schema,
            &format!("/$defs/filters/properties/{field}/type"),
            "string",
        );
        assert_schema_type_contains(
            "worklist filter null option",
            &schema,
            &format!("/$defs/filters/properties/{field}/type"),
            "null",
        );
    }
    assert_enum_contains_all(
        "worklist",
        &schema,
        "/$defs/filters/properties/status/enum",
        &["matched", "new", "baseline_debt"],
    );
    assert_enum_contains_all(
        "worklist",
        &schema,
        "/$defs/filters/properties/risk/enum",
        &["low", "medium", "high"],
    );
    assert_enum_contains_all(
        "worklist",
        &schema,
        "/$defs/filters/properties/difficulty/enum",
        &["small", "medium"],
    );
    for field in ["baseline_debt", "broad_scope", "missing_evidence"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/filters/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("boolean"),
            "worklist filter {field} should be boolean"
        );
    }

    let summary = required_schema_pointer("worklist", &schema, "/$defs/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "worklist summary should reject unknown fields"
    );
    assert_required_fields(
        "worklist summary",
        summary,
        &[
            "work_items",
            "high",
            "medium",
            "low",
            "small_difficulty",
            "medium_difficulty",
        ],
    );
    for field in [
        "work_items",
        "high",
        "medium",
        "low",
        "small_difficulty",
        "medium_difficulty",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "worklist summary {field} should be an integer"
        );
    }

    let work_item = required_schema_pointer("worklist", &schema, "/$defs/work_item");
    assert_eq!(
        work_item
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "worklist items should reject unknown fields"
    );
    assert_required_fields(
        "worklist item",
        work_item,
        &[
            "id",
            "kind",
            "risk",
            "difficulty",
            "status",
            "allow_id",
            "finding_index",
            "path",
            "source_package",
            "message",
            "suggested_actions",
            "proof_commands",
        ],
    );
    assert_enum_contains_all(
        "worklist",
        &schema,
        "/$defs/work_item/properties/exception_kind/enum",
        &["panic", "unsafe", "lint_exception", "non_rust_file"],
    );
    assert_schema_type_contains(
        "worklist item source_package",
        &schema,
        "/$defs/work_item/properties/source_package/type",
        "string",
    );
    assert_schema_type_contains(
        "worklist item source_package",
        &schema,
        "/$defs/work_item/properties/source_package/type",
        "null",
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/proof_commands/items/pattern")
            .and_then(Value::as_str),
        Some("^cargo-allow "),
        "worklist proof commands should stay cargo-allow first"
    );
}
