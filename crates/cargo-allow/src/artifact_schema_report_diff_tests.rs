use crate::artifact_schema_expectations::structural_identity_fields;
use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, enum_strings,
    match_status_enum, parse_schema, required_schema_pointer,
};
use allow_diff::{FindingPostureKind, PolicyChangeKind, PolicyChangeSeverity};
use serde_json::Value;

#[test]
fn report_schema_locks_diff_posture_extension_contract() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/allOf/0/if/required/0")
            .and_then(Value::as_str),
        Some("diff"),
        "report schema should condition on the diff extension"
    );
    assert_eq!(
        schema
            .pointer("/allOf/0/then/properties/command/const")
            .and_then(Value::as_str),
        Some("diff"),
        "report schema should allow the diff extension only on diff reports"
    );
    assert_eq!(
        schema
            .pointer("/allOf/0/then/additionalProperties")
            .and_then(Value::as_bool),
        None,
        "report diff conditional should not reject normal report fields"
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
    for field in [
        "broken_evidence_links",
        "missing_evidence",
        "weak_evidence_references",
        "evidence_added",
        "evidence_removed",
        "link_added",
        "link_removed",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff_summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "report diff summary optional {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff_summary/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "report diff summary optional {field} minimum"
        );
    }
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
    assert_schema_type_equals(
        "report finding_posture_change source_package",
        &schema,
        "/$defs/finding_posture_change/properties/source_package/type",
        &["string", "null"],
    );
    for field in ["line", "column"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/finding_posture_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("integer"),
            "report finding posture {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/finding_posture_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "report finding posture {field} minimum"
        );
    }
    assert_eq!(
        schema
            .pointer("/$defs/finding_posture_change/properties/source_package/minLength")
            .and_then(Value::as_u64),
        Some(1),
        "report finding posture source_package minLength"
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding_posture_change/properties/identity/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/structural_identity"),
        "report finding posture identity should use structural identity"
    );
    assert_required_fields(
        "report structural_identity",
        required_schema_pointer("report", &schema, "/$defs/structural_identity"),
        &structural_identity_fields(),
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
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/exception_identity/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/exception_identity_change"),
        "report policy changes should use exception identity change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/selector_identity/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/selector_identity_change"),
        "report policy changes should use selector identity change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/selector_precision/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/selector_precision_change"),
        "report policy changes should use selector precision rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/scope/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/scope_change"),
        "report policy changes should use scope change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/occurrence_limit/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/occurrence_limit_change"),
        "report policy changes should use occurrence limit rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/lifecycle/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/lifecycle_change"),
        "report policy changes should use lifecycle change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/evidence/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_change"),
        "report policy changes should use evidence change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/metadata/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/metadata_change"),
        "report policy changes should use metadata change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/requirement/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/requirement_change"),
        "report policy changes should use requirement change rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/policy_change/properties/policy_status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/policy_status_change"),
        "report policy changes should use policy status change rows"
    );
}
