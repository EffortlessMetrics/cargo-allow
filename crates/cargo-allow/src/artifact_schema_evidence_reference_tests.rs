use crate::artifact_schema_support::{parse_schema, required_schema_pointer};
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
    }
}

fn assert_schema_enum_or_ref_equals(name: &str, schema: &Value, pointer: &str, expected: &[&str]) {
    let actual = schema_enum_or_ref_values(name, schema, pointer);
    let expected = expected.iter().map(|item| (*item).to_string()).collect();
    assert_eq!(actual, expected, "{name} {pointer} enum values");
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
