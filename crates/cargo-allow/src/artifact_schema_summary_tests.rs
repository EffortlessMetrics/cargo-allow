use crate::artifact_schema_support::{
    assert_required_fields, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn common_schema_summary_fragments_keep_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    let counts = required_schema_pointer("common", &schema, "/$defs/counts");
    assert_eq!(
        counts.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common counts should reject unknown fields"
    );
    let counts_required_fields = [
        "matched",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];
    assert_required_fields("common counts", counts, &counts_required_fields);
    assert_integer_counter_fields(
        &schema,
        "counts",
        &[
            "matched",
            "new",
            "expired",
            "review_due",
            "stale",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt",
            "policy_baseline_debt",
            "policy_missing_evidence",
            "broken_evidence_links",
            "weak_evidence_references",
        ],
    );

    let summary = required_schema_pointer("common", &schema, "/$defs/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common summary should reject unknown fields"
    );
    let summary_required_fields = [
        "findings",
        "outcomes",
        "matched",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];
    assert_required_fields("common summary", summary, &summary_required_fields);
    assert_integer_counter_fields(
        &schema,
        "summary",
        &[
            "findings",
            "outcomes",
            "matched",
            "new",
            "expired",
            "review_due",
            "stale",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt",
            "policy_baseline_debt",
            "policy_missing_evidence",
            "broken_evidence_links",
            "weak_evidence_references",
        ],
    );

    let trend = required_schema_pointer("common", &schema, "/$defs/trend");
    assert_eq!(
        trend.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common trend should reject unknown fields"
    );
    let trend_required_fields = [
        "review_items",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];
    assert_required_fields("common trend", trend, &trend_required_fields);
    assert_integer_counter_fields(
        &schema,
        "trend",
        &[
            "review_items",
            "new",
            "expired",
            "review_due",
            "stale",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt",
            "policy_missing_evidence",
            "broken_evidence_links",
            "weak_evidence_references",
        ],
    );
}

fn assert_integer_counter_fields(schema: &Value, fragment: &str, fields: &[&str]) {
    for field in fields {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/{fragment}/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "common {fragment} {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/{fragment}/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "common {fragment} {field} minimum"
        );
    }
}
