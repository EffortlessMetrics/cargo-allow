use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, enum_strings, parse_schema, required_schema_pointer,
};
use allow_diff::{
    EvidenceChangeField, LifecycleChangeField, MetadataChangeField, RequirementChangeField,
    ScopeChangeField,
};
use serde_json::Value;

#[test]
fn report_schema_locks_diff_policy_detail_contracts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
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
}
