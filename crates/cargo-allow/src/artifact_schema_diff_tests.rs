use crate::artifact_schema_expectations::{
    finding_posture_kinds, policy_change_kinds, policy_change_severities,
};
use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, governed_kind_enum,
    parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn common_schema_diff_fragments_keep_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    let diff = required_schema_pointer("common", &schema, "/$defs/diff");
    assert_eq!(
        diff.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common diff should reject unknown fields"
    );
    assert_required_fields(
        "common diff",
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
        "common diff net_posture",
        &schema,
        "/$defs/diff/properties/net_posture/enum",
        &["worse", "review-required", "improved", "unchanged"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/reviewer_action/type")
            .and_then(Value::as_str),
        Some("string"),
        "common diff reviewer_action type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/reviewer_action/minLength")
            .and_then(Value::as_u64),
        Some(1),
        "common diff reviewer_action minLength"
    );
    for (field, reference) in [
        ("summary", "#/$defs/diff_summary"),
        ("finding_changes/items", "#/$defs/finding_posture_change"),
        ("policy_changes/items", "#/$defs/policy_change"),
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff/properties/{field}/$ref"))
                .and_then(Value::as_str),
            Some(reference),
            "common diff {field} ref"
        );
    }
    for field in ["finding_changes", "policy_changes"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("array"),
            "common diff {field} type"
        );
    }

    let diff_summary = required_schema_pointer("common", &schema, "/$defs/diff_summary");
    assert_eq!(
        diff_summary
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common diff_summary should reject unknown fields"
    );
    let diff_summary_fields = [
        "current_failures",
        "new_findings",
        "removed_findings",
        "policy_failures",
        "policy_review_items",
        "policy_improvements",
    ];
    assert_required_fields("common diff_summary", diff_summary, &diff_summary_fields);
    for field in diff_summary_fields {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff_summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "common diff_summary {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff_summary/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "common diff_summary {field} minimum"
        );
    }

    assert_enum_equals(
        "common finding posture kinds",
        &schema,
        "/$defs/finding_posture_kind/enum",
        &finding_posture_kinds(),
    );
    let finding_posture_change =
        required_schema_pointer("common", &schema, "/$defs/finding_posture_change");
    assert_eq!(
        finding_posture_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common finding_posture_change should reject unknown fields"
    );
    assert_required_fields(
        "common finding_posture_change",
        finding_posture_change,
        &["change", "key", "kind", "family", "path"],
    );
    assert_enum_equals(
        "common finding_posture_change change",
        &schema,
        "/$defs/finding_posture_change/properties/change/enum",
        &["new", "removed"],
    );
    assert_enum_equals(
        "common finding_posture_change kind",
        &schema,
        "/$defs/finding_posture_change/properties/kind/enum",
        &governed_kind_enum(),
    );
    for field in ["key", "path"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/finding_posture_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "common finding posture {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/finding_posture_change/properties/{field}/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common finding posture {field} minLength"
        );
    }
    assert_schema_type_equals(
        "common finding_posture_change family",
        &schema,
        "/$defs/finding_posture_change/properties/family/type",
        &["string", "null"],
    );

    assert_enum_equals(
        "common policy change severities",
        &schema,
        "/$defs/policy_change_severity/enum",
        &policy_change_severities(),
    );
    assert_enum_equals(
        "common policy change kinds",
        &schema,
        "/$defs/policy_change_kind/enum",
        &policy_change_kinds(),
    );
    let policy_change = required_schema_pointer("common", &schema, "/$defs/policy_change");
    assert_eq!(
        policy_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common policy_change should reject unknown fields"
    );
    assert_required_fields(
        "common policy_change",
        policy_change,
        &["severity", "allow_id", "kind", "message"],
    );
    assert_enum_equals(
        "common policy_change severity",
        &schema,
        "/$defs/policy_change/properties/severity/enum",
        &policy_change_severities(),
    );
    assert_enum_equals(
        "common policy_change kind",
        &schema,
        "/$defs/policy_change/properties/kind/enum",
        &policy_change_kinds(),
    );
    for field in ["allow_id", "message"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/policy_change/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("string"),
            "common policy_change {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/policy_change/properties/{field}/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common policy_change {field} minLength"
        );
    }
    for (field, reference) in [
        ("exception_identity", "#/$defs/exception_identity_change"),
        ("selector_identity", "#/$defs/selector_identity_change"),
        ("selector_precision", "#/$defs/selector_precision_change"),
        ("scope", "#/$defs/scope_change"),
        ("occurrence_limit", "#/$defs/occurrence_limit_change"),
        ("lifecycle", "#/$defs/lifecycle_change"),
        ("evidence", "#/$defs/evidence_change"),
        ("metadata", "#/$defs/metadata_change"),
        ("requirement", "#/$defs/requirement_change"),
        ("policy_status", "#/$defs/policy_status_change"),
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/policy_change/properties/{field}/$ref"))
                .and_then(Value::as_str),
            Some(reference),
            "common policy_change {field} ref"
        );
    }
}
