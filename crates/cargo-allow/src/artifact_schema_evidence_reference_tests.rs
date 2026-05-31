use crate::artifact_schema_expectations::evidence_change_fields;
use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn evidence_reference_status_vocabularies_match_policy() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    let explain = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );
    let worklist = parse_schema(
        "worklist",
        include_str!("../../../docs/schemas/worklist.schema.json"),
    );
    let evidence_reference_statuses = allow_policy::EvidenceReferenceStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<_>>();
    let evidence_reference_categories = allow_policy::EvidenceReferenceCategory::ALL
        .iter()
        .map(|category| category.as_str())
        .collect::<Vec<_>>();

    for (schema_name, schema) in [
        ("common", &common),
        ("explain", &explain),
        ("worklist", &worklist),
    ] {
        assert_schema_enum_or_ref_equals(
            schema_name,
            schema,
            "/$defs/evidence_reference/properties/status",
            &evidence_reference_statuses,
        );
        assert_schema_enum_or_ref_equals(
            schema_name,
            schema,
            "/$defs/evidence_reference/properties/category",
            &evidence_reference_categories,
        );
        assert_evidence_category_descriptions_are_machine_contracts(schema_name, schema);
    }
}

#[test]
fn common_schema_evidence_fragments_keep_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    let canonical_evidence_prefixes =
        allow_policy::canonical_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/canonical_evidence_prefix/enum",
        &canonical_evidence_prefixes,
    );
    let recognized_evidence_prefixes =
        allow_policy::recognized_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/recognized_evidence_prefix/enum",
        &recognized_evidence_prefixes,
    );
    let local_file_evidence_prefixes =
        allow_policy::local_file_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/local_file_evidence_prefix/enum",
        &local_file_evidence_prefixes,
    );
    let traceability_evidence_prefixes =
        allow_policy::traceability_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/traceability_evidence_prefix/enum",
        &traceability_evidence_prefixes,
    );
    let evidence_reference_statuses = allow_policy::EvidenceReferenceStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<_>>();
    let evidence_reference_categories = allow_policy::EvidenceReferenceCategory::ALL
        .iter()
        .map(|category| category.as_str())
        .collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/evidence_reference_status/enum",
        &evidence_reference_statuses,
    );
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/evidence_reference_category/enum",
        &evidence_reference_categories,
    );

    let evidence_reference =
        required_schema_pointer("common", &schema, "/$defs/evidence_reference");
    assert_eq!(
        evidence_reference
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common evidence_reference should reject unknown fields"
    );
    assert_required_fields(
        "common evidence_reference",
        evidence_reference,
        &["raw", "prefix", "target", "status", "message"],
    );
    assert_schema_type_equals(
        "common evidence_reference prefix",
        &schema,
        "/$defs/evidence_reference/properties/prefix/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common evidence_reference target",
        &schema,
        "/$defs/evidence_reference/properties/target/type",
        &["string", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_reference/properties/status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference_status"),
        "common evidence_reference status should use the shared status vocabulary"
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_reference/properties/category/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference_category"),
        "common evidence_reference category should use the shared category vocabulary"
    );

    assert_enum_equals(
        "common evidence fields",
        &schema,
        "/$defs/evidence_change_field/enum",
        &evidence_change_fields(),
    );
    let evidence_change = required_schema_pointer("common", &schema, "/$defs/evidence_change");
    assert_eq!(
        evidence_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common evidence_change should reject unknown fields"
    );
    assert_required_fields(
        "common evidence_change",
        evidence_change,
        &["field", "removed", "added"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_change_field"),
        "common evidence_change field should use the shared evidence field vocabulary"
    );
    for field in ["removed", "added"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/evidence_change/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("array"),
            "common evidence_change {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_change/properties/{field}/items/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "common evidence_change {field} item type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_change/properties/{field}/items/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common evidence_change {field} item minLength"
        );
    }
}

fn assert_schema_enum_or_ref_equals(name: &str, schema: &Value, pointer: &str, expected: &[&str]) {
    let actual = schema_enum_or_ref_values(name, schema, pointer);
    let expected = expected.iter().map(|item| (*item).to_string()).collect();
    assert_eq!(actual, expected, "{name} {pointer} enum values");
}

fn assert_evidence_category_descriptions_are_machine_contracts(name: &str, schema: &Value) {
    for pointer in [
        "/$defs/evidence_reference_category/description",
        "/$defs/evidence_reference/properties/category/description",
    ] {
        let description = schema
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("{name} {pointer} should describe category"))
            });
        assert!(
            description.contains("structured diagnostic category")
                || description.contains("Structured diagnostic category"),
            "{name} {pointer} should describe category as a structured contract"
        );
        assert!(
            description.contains("Human renderers may map"),
            "{name} {pointer} should document human-renderer label mapping"
        );
    }
}

fn schema_enum_or_ref_values(name: &str, schema: &Value, pointer: &str) -> BTreeSet<String> {
    let value = required_schema_pointer(name, schema, pointer);
    if let Some(items) = value.get("enum").and_then(Value::as_array) {
        return items
            .iter()
            .map(|item| {
                item.as_str()
                    .unwrap_or_else(|| {
                        std::panic::panic_any(format!(
                            "{name} {pointer} enum entries should be strings"
                        ))
                    })
                    .to_string()
            })
            .collect();
    }

    let Some(ref_pointer) = value
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'))
    else {
        std::panic::panic_any(format!("{name} {pointer} should define enum or local $ref"));
    };
    let referenced = required_schema_pointer(name, schema, ref_pointer);
    let Some(items) = referenced.get("enum").and_then(Value::as_array) else {
        std::panic::panic_any(format!(
            "{name} {pointer} referenced schema should define enum"
        ));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{name} {pointer} referenced enum entries should be strings"
                    ))
                })
                .to_string()
        })
        .collect()
}
