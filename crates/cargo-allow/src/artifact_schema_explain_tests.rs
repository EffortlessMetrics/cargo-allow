use crate::artifact_schema_support::{
    GOVERNED_KIND_ENUM, assert_enum_contains_all, assert_enum_equals, assert_required_fields,
    assert_schema_type_contains, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn explain_schema_locks_entry_status_and_next_steps_contract() {
    let schema = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );

    assert_required_fields(
        "explain",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "allow_entry",
            "summary",
            "evidence_references",
            "current_findings",
            "match_outcomes",
            "next",
        ],
    );

    let summary = required_schema_pointer("explain", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "explain summary should reject unknown fields"
    );
    assert_required_fields(
        "explain summary",
        summary,
        &["current_status", "current_matches", "match_outcomes"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/current_status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/match_status"),
        "explain summary current_status should use match_status"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/current_matches/type")
            .and_then(Value::as_str),
        Some("integer"),
        "explain current_matches should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/match_outcomes/type")
            .and_then(Value::as_str),
        Some("integer"),
        "explain match_outcomes should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/evidence_references/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference"),
        "explain evidence references should use evidence reference rows"
    );
    let evidence_reference =
        required_schema_pointer("explain", &schema, "/$defs/evidence_reference");
    assert_eq!(
        evidence_reference
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "explain evidence references should reject unknown fields"
    );
    assert_required_fields(
        "explain evidence reference",
        evidence_reference,
        &["raw", "prefix", "target", "status", "message"],
    );
    assert_enum_contains_all(
        "explain",
        &schema,
        "/$defs/evidence_reference/properties/status/enum",
        &[
            "local_file_present",
            "local_file_missing",
            "invalid_local_path",
            "traceability_only",
            "unstructured",
        ],
    );

    assert_enum_equals(
        "explain allow_entry kind",
        &schema,
        "/$defs/allow_entry/properties/kind/enum",
        GOVERNED_KIND_ENUM,
    );
    assert_eq!(
        schema
            .pointer("/properties/current_findings/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/current_finding"),
        "explain current findings should use current finding rows"
    );
    let current_finding = required_schema_pointer("explain", &schema, "/$defs/current_finding");
    assert_eq!(
        current_finding
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "explain current findings should reject unknown fields"
    );
    assert_required_fields(
        "explain current finding",
        current_finding,
        &[
            "status",
            "kind",
            "family",
            "path",
            "line",
            "column",
            "source_package",
            "identity",
            "message",
        ],
    );
    assert_enum_equals(
        "explain current finding kind",
        &schema,
        "/$defs/current_finding/properties/kind/enum",
        GOVERNED_KIND_ENUM,
    );
    assert_schema_type_contains(
        "explain current finding source_package",
        &schema,
        "/$defs/current_finding/properties/source_package/type",
        "string",
    );
    assert_schema_type_contains(
        "explain current finding source_package",
        &schema,
        "/$defs/current_finding/properties/source_package/type",
        "null",
    );
    assert_eq!(
        schema
            .pointer("/properties/match_outcomes/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/match_outcome"),
        "explain match outcomes should use match outcome rows"
    );

    let next = required_schema_pointer("explain", &schema, "/properties/next");
    assert_eq!(
        next.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "explain next should reject unknown fields"
    );
    assert_required_fields(
        "explain next",
        next,
        &["suggested_actions", "proof_commands"],
    );
}
