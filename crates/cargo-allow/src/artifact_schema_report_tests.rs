use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, match_status_enum, parse_schema,
    required_schema_pointer,
};
use allow_diff::{FindingPostureKind, PolicyChangeKind, PolicyChangeSeverity};
use serde_json::Value;

#[test]
fn report_schema_allows_optional_policy_baseline_debt_summary_count() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let policy_baseline_debt = required_schema_pointer(
        "report",
        &schema,
        "/$defs/summary/properties/policy_baseline_debt",
    );
    assert_eq!(
        policy_baseline_debt.get("type").and_then(Value::as_str),
        Some("integer"),
        "report policy_baseline_debt count type"
    );
    assert_eq!(
        policy_baseline_debt.get("minimum").and_then(Value::as_u64),
        Some(0),
        "report policy_baseline_debt count minimum"
    );
}

#[test]
fn report_schema_allows_optional_broken_evidence_link_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/broken_evidence_links",
        "/$defs/trend/properties/broken_evidence_links",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
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
    assert_enum_equals(
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
    assert_enum_equals(
        "report match status",
        &schema,
        "/$defs/match_status/enum",
        &match_status_enum(),
    );
    assert_required_fields(
        "report finding posture change",
        required_schema_pointer("report", &schema, "/$defs/finding_posture_change"),
        &["change", "key", "kind", "family", "path"],
    );
    assert_enum_equals(
        "report",
        &schema,
        "/$defs/finding_posture_change/properties/change/enum",
        &enum_strings(FindingPostureKind::ALL, FindingPostureKind::as_str),
    );
    assert_required_fields(
        "report policy change",
        required_schema_pointer("report", &schema, "/$defs/policy_change"),
        &["severity", "allow_id", "kind", "message"],
    );
    assert_enum_equals(
        "report",
        &schema,
        "/$defs/policy_change/properties/severity/enum",
        &enum_strings(PolicyChangeSeverity::ALL, PolicyChangeSeverity::as_str),
    );
    assert_enum_equals(
        "report",
        &schema,
        "/$defs/policy_change/properties/kind/enum",
        &enum_strings(PolicyChangeKind::ALL, PolicyChangeKind::as_str),
    );
}

fn enum_strings<T: Copy>(values: &[T], as_str: impl Fn(T) -> &'static str) -> Vec<&'static str> {
    values.iter().copied().map(as_str).collect()
}
