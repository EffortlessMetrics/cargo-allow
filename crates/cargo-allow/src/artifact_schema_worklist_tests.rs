use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, governed_kind_enum,
    match_status_enum, parse_schema, required_schema_pointer,
};
use crate::worklist::{DIFFICULTY_LEVELS, RISK_LEVELS, WORK_ITEM_KINDS};
use serde_json::Value;
use std::collections::BTreeSet;

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
    assert!(
        filters.get("required").is_none(),
        "worklist filter fields should stay optional for worklist.v1 compatibility"
    );
    assert_worklist_filter_properties(filters);
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
        assert_schema_type_equals(
            "worklist filter string option",
            &schema,
            &format!("/$defs/filters/properties/{field}/type"),
            &["string", "null"],
        );
    }
    assert_nullable_string_enum_equals(
        "worklist",
        &schema,
        "/$defs/filters/properties/status/enum",
        &match_status_enum(),
    );
    assert_nullable_string_enum_equals(
        "worklist",
        &schema,
        "/$defs/filters/properties/risk/enum",
        RISK_LEVELS,
    );
    assert_nullable_string_enum_equals(
        "worklist",
        &schema,
        "/$defs/filters/properties/difficulty/enum",
        DIFFICULTY_LEVELS,
    );
    for field in [
        "baseline_debt",
        "broad_scope",
        "missing_evidence",
        "broken_evidence",
        "weak_evidence",
    ] {
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
    assert!(
        !summary
            .get("required")
            .and_then(Value::as_array)
            .unwrap_or_else(|| std::panic::panic_any(
                "worklist summary required should be an array"
            ))
            .iter()
            .any(|field| field.as_str() == Some("item_kinds")),
        "worklist summary item_kinds should stay optional for worklist.v1 compatibility"
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
    assert_eq!(
        schema
            .pointer("/$defs/summary/properties/item_kinds/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/item_kind_counts"),
        "worklist summary item_kinds should use item-kind count schema"
    );
    let item_kind_counts = required_schema_pointer("worklist", &schema, "/$defs/item_kind_counts");
    assert_eq!(
        item_kind_counts
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "worklist item_kind_counts should reject unknown queue kinds"
    );
    assert!(
        item_kind_counts.get("required").is_none(),
        "worklist item_kind_counts fields should stay optional"
    );
    assert_worklist_item_kind_count_properties(item_kind_counts);
    for kind in WORK_ITEM_KINDS {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/item_kind_counts/properties/{kind}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "worklist item_kind_counts {kind} should be an integer"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/item_kind_counts/properties/{kind}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "worklist item_kind_counts {kind} minimum"
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
            "message",
            "suggested_actions",
            "proof_commands",
        ],
    );
    let work_item_required = work_item
        .get("required")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("worklist item required should be an array"))
        .iter()
        .map(|field| {
            field.as_str().unwrap_or_else(|| {
                std::panic::panic_any("worklist item required entries should be strings")
            })
        })
        .collect::<BTreeSet<_>>();
    assert!(
        !work_item_required.contains("evidence_reference"),
        "worklist evidence_reference should stay optional for worklist.v1 compatibility"
    );
    assert!(
        !work_item_required.contains("selector_precision"),
        "worklist selector_precision should stay optional for worklist.v1 compatibility"
    );
    assert_schema_type_equals(
        "worklist item selector_precision",
        &schema,
        "/$defs/work_item/properties/selector_precision/type",
        &["integer", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/selector_precision/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "worklist selector_precision minimum"
    );
    assert_enum_equals(
        "worklist item risk",
        &schema,
        "/$defs/work_item/properties/risk/enum",
        RISK_LEVELS,
    );
    assert_enum_equals(
        "worklist item difficulty",
        &schema,
        "/$defs/work_item/properties/difficulty/enum",
        DIFFICULTY_LEVELS,
    );
    assert_enum_equals(
        "worklist item status",
        &schema,
        "/$defs/work_item/properties/status/enum",
        &match_status_enum(),
    );
    assert_enum_equals(
        "worklist item kind",
        &schema,
        "/$defs/work_item/properties/kind/enum",
        WORK_ITEM_KINDS,
    );
    assert_enum_equals(
        "worklist",
        &schema,
        "/$defs/work_item/properties/exception_kind/enum",
        &governed_kind_enum(),
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/source_package/type")
            .and_then(Value::as_str),
        Some("string"),
        "worklist item source_package should be a string when present"
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/candidate_ids/type")
            .and_then(Value::as_str),
        Some("array"),
        "worklist item candidate_ids should be an array"
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/candidate_ids/items/type")
            .and_then(Value::as_str),
        Some("string"),
        "worklist item candidate_ids should contain strings"
    );
    assert_schema_type_equals(
        "worklist item path",
        &schema,
        "/$defs/work_item/properties/path/type",
        &["string", "null"],
    );
    let path_description = schema
        .pointer("/$defs/work_item/properties/path/description")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            std::panic::panic_any("worklist item path should document source-tree semantics")
        });
    assert!(
        path_description.contains("Source-tree path"),
        "worklist item path should be documented as source-tree scoped"
    );
    assert!(
        path_description.contains("weak_evidence_reference"),
        "worklist item path should document why weak evidence references use null"
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/evidence_reference/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference"),
        "worklist evidence references should use evidence reference rows"
    );
    let evidence_reference =
        required_schema_pointer("worklist", &schema, "/$defs/evidence_reference");
    assert_eq!(
        evidence_reference
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "worklist evidence references should reject unknown fields"
    );
    assert_required_fields(
        "worklist evidence reference",
        evidence_reference,
        &["raw", "prefix", "target", "status", "message"],
    );
    let evidence_statuses = allow_policy::EvidenceReferenceStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<_>>();
    let evidence_categories = allow_policy::EvidenceReferenceCategory::ALL
        .iter()
        .map(|category| category.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        schema
            .pointer("/$defs/evidence_reference/properties/status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference_status"),
        "worklist evidence reference status should use the shared status definition"
    );
    assert_enum_equals(
        "worklist evidence reference status",
        &schema,
        "/$defs/evidence_reference_status/enum",
        &evidence_statuses,
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_reference/properties/category/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference_category"),
        "worklist evidence reference category should use the shared category definition"
    );
    assert_enum_equals(
        "worklist evidence reference category",
        &schema,
        "/$defs/evidence_reference_category/enum",
        &evidence_categories,
    );
    assert_eq!(
        schema
            .pointer("/$defs/work_item/properties/proof_commands/items/pattern")
            .and_then(Value::as_str),
        Some("^cargo-allow "),
        "worklist proof commands should stay cargo-allow first"
    );
    let proof_command_description = schema
        .pointer("/$defs/work_item/properties/proof_commands/items/description")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            std::panic::panic_any("worklist proof commands should document their boundary")
        });
    assert!(
        proof_command_description.contains("standalone cargo-allow commands"),
        "worklist proof commands should document that they are standalone cargo-allow commands"
    );
    assert!(
        proof_command_description.contains("does not execute"),
        "worklist proof commands should document that cargo-allow does not execute them"
    );
}

fn assert_nullable_string_enum_equals(
    name: &str,
    schema: &Value,
    pointer: &str,
    expected_strings: &[&str],
) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} {pointer} should be an enum array"));
    };
    let actual = items
        .iter()
        .map(|item| match item {
            Value::String(value) => value.clone(),
            Value::Null => "<null>".to_string(),
            _ => std::panic::panic_any(format!(
                "{name} {pointer} entries should be strings or null"
            )),
        })
        .collect::<BTreeSet<_>>();
    let expected = expected_strings
        .iter()
        .map(|item| (*item).to_string())
        .chain(std::iter::once("<null>".to_string()))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{name} {pointer} enum values");
}

fn assert_worklist_filter_properties(filters: &Value) {
    let Some(properties) = filters.get("properties").and_then(Value::as_object) else {
        std::panic::panic_any("worklist filters properties should be an object");
    };
    let actual = properties
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = [
        "kind",
        "family",
        "item_kind",
        "status",
        "allow_id",
        "path",
        "source_package",
        "owner",
        "classification",
        "baseline_debt",
        "broad_scope",
        "risk",
        "difficulty",
        "missing_evidence",
        "broken_evidence",
        "weak_evidence",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected, "worklist filter schema properties");
}

fn assert_worklist_item_kind_count_properties(item_kind_counts: &Value) {
    let Some(properties) = item_kind_counts
        .get("properties")
        .and_then(Value::as_object)
    else {
        std::panic::panic_any("worklist item_kind_counts properties should be an object");
    };
    let actual = properties
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = WORK_ITEM_KINDS.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(
        actual, expected,
        "worklist item_kind_counts schema properties"
    );
}
