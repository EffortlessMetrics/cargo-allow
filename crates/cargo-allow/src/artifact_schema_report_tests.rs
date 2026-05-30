use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, match_status_enum, parse_schema,
    required_schema_pointer,
};
use allow_diff::{
    EvidenceChangeField, FindingPostureKind, LifecycleChangeField, MetadataChangeField,
    PolicyChangeKind, PolicyChangeSeverity, RequirementChangeField, ScopeChangeField,
};
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
    let policy_status_change =
        required_schema_pointer("report", &schema, "/$defs/policy_status_change");
    assert_eq!(
        policy_status_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report policy status changes should reject unknown fields"
    );
    assert_required_fields(
        "report policy status change",
        policy_status_change,
        &["before", "after"],
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/policy_status_change/properties/{field}/type/0"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "report policy status {field} first type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/policy_status_change/properties/{field}/type/1"
                ))
                .and_then(Value::as_str),
            Some("null"),
            "report policy status {field} second type"
        );
    }
    let requirement_change =
        required_schema_pointer("report", &schema, "/$defs/requirement_change");
    assert_eq!(
        requirement_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report requirement changes should reject unknown fields"
    );
    assert_required_fields(
        "report requirement change",
        requirement_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/requirement_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/requirement_change_field"),
        "report requirement changes should use the requirement field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/requirement_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("boolean"),
            "report requirement {field} type"
        );
    }
    assert_enum_equals(
        "report requirement fields",
        &schema,
        "/$defs/requirement_change_field/enum",
        &enum_strings(RequirementChangeField::ALL, RequirementChangeField::as_str),
    );
    let metadata_change = required_schema_pointer("report", &schema, "/$defs/metadata_change");
    assert_eq!(
        metadata_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report metadata changes should reject unknown fields"
    );
    assert_required_fields(
        "report metadata change",
        metadata_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/metadata_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/metadata_change_field"),
        "report metadata changes should use the metadata field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/metadata_change/properties/{field}/type/0"))
                .and_then(Value::as_str),
            Some("string"),
            "report metadata {field} first type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/metadata_change/properties/{field}/type/1"))
                .and_then(Value::as_str),
            Some("null"),
            "report metadata {field} second type"
        );
    }
    assert_enum_equals(
        "report metadata fields",
        &schema,
        "/$defs/metadata_change_field/enum",
        &enum_strings(MetadataChangeField::ALL, MetadataChangeField::as_str),
    );
    let evidence_change = required_schema_pointer("report", &schema, "/$defs/evidence_change");
    assert_eq!(
        evidence_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report evidence changes should reject unknown fields"
    );
    assert_required_fields(
        "report evidence change",
        evidence_change,
        &["field", "removed", "added"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_change_field"),
        "report evidence changes should use the evidence field vocabulary"
    );
    for field in ["removed", "added"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/evidence_change/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("array"),
            "report evidence {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_change/properties/{field}/items/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "report evidence {field} item type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_change/properties/{field}/items/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "report evidence {field} item minLength"
        );
    }
    assert_enum_equals(
        "report evidence fields",
        &schema,
        "/$defs/evidence_change_field/enum",
        &enum_strings(EvidenceChangeField::ALL, EvidenceChangeField::as_str),
    );
    let lifecycle_change = required_schema_pointer("report", &schema, "/$defs/lifecycle_change");
    assert_eq!(
        lifecycle_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report lifecycle changes should reject unknown fields"
    );
    assert_required_fields(
        "report lifecycle change",
        lifecycle_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/lifecycle_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/lifecycle_change_field"),
        "report lifecycle changes should use the lifecycle field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/lifecycle_change/properties/{field}/type/0"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "report lifecycle {field} first type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/lifecycle_change/properties/{field}/type/1"
                ))
                .and_then(Value::as_str),
            Some("null"),
            "report lifecycle {field} second type"
        );
    }
    assert_enum_equals(
        "report lifecycle fields",
        &schema,
        "/$defs/lifecycle_change_field/enum",
        &enum_strings(LifecycleChangeField::ALL, LifecycleChangeField::as_str),
    );
    let occurrence_limit =
        required_schema_pointer("report", &schema, "/$defs/occurrence_limit_change");
    assert_eq!(
        occurrence_limit
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report occurrence limit changes should reject unknown fields"
    );
    assert_required_fields(
        "report occurrence limit change",
        occurrence_limit,
        &["before", "after"],
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/occurrence_limit_change/properties/{field}/type/0"
                ))
                .and_then(Value::as_str),
            Some("integer"),
            "report occurrence limit {field} first type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/occurrence_limit_change/properties/{field}/type/1"
                ))
                .and_then(Value::as_str),
            Some("null"),
            "report occurrence limit {field} second type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/occurrence_limit_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "report occurrence limit {field} minimum"
        );
    }
    let scope_change = required_schema_pointer("report", &schema, "/$defs/scope_change");
    assert_eq!(
        scope_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report scope changes should reject unknown fields"
    );
    assert_required_fields(
        "report scope change",
        scope_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/scope_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/scope_change_field"),
        "report scope changes should use the scope field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/scope_change/properties/{field}/type/0"))
                .and_then(Value::as_str),
            Some("string"),
            "report scope {field} first type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/scope_change/properties/{field}/type/1"))
                .and_then(Value::as_str),
            Some("null"),
            "report scope {field} second type"
        );
    }
    assert_enum_equals(
        "report scope fields",
        &schema,
        "/$defs/scope_change_field/enum",
        &enum_strings(ScopeChangeField::ALL, ScopeChangeField::as_str),
    );
    let selector_precision =
        required_schema_pointer("report", &schema, "/$defs/selector_precision_change");
    assert_eq!(
        selector_precision
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "report selector precision changes should reject unknown fields"
    );
    assert_required_fields(
        "report selector precision change",
        selector_precision,
        &["before", "after", "removed_fields", "added_fields"],
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("integer"),
            "report selector precision {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "report selector precision {field} minimum"
        );
    }
    for field in ["removed_fields", "added_fields"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/items/$ref"
                ))
                .and_then(Value::as_str),
            Some("#/$defs/selector_precision_field"),
            "report selector precision {field} should use the field vocabulary"
        );
    }
    assert_enum_equals(
        "report selector precision fields",
        &schema,
        "/$defs/selector_precision_field/enum",
        &[
            "path",
            "glob",
            "family",
            "ast_kind",
            "container",
            "callee",
            "macro_name",
            "lint",
            "symbol",
            "receiver_fingerprint",
            "target_fingerprint",
            "normalized_snippet_hash",
            "occurrence_limit",
        ],
    );
}

fn enum_strings<T: Copy>(values: &[T], as_str: impl Fn(T) -> &'static str) -> Vec<&'static str> {
    values.iter().copied().map(as_str).collect()
}
