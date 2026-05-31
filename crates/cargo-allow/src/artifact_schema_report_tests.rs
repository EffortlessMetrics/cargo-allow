use crate::artifact_schema_support::{assert_enum_equals, parse_schema, required_schema_pointer};
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
fn report_schema_locks_top_level_status_vocabulary() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_enum_equals(
        "report status",
        &schema,
        "/properties/status/enum",
        allow_report::ARTIFACT_STATUSES,
    );
}

#[test]
fn report_schema_locks_report_command_producers() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_enum_equals(
        "report command",
        &schema,
        "/properties/command/enum",
        allow_report::REPORT_COMMANDS,
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
fn report_schema_allows_optional_weak_evidence_reference_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/weak_evidence_references",
        "/$defs/trend/properties/weak_evidence_references",
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
fn report_schema_allows_optional_policy_missing_evidence_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/policy_missing_evidence",
        "/$defs/trend/properties/policy_missing_evidence",
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
