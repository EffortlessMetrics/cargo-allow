use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, match_status_enum, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;

#[test]
fn why_schema_locks_finding_outcome_and_candidates_contract() {
    let schema = parse_schema("why", include_str!("../../../docs/schemas/why.schema.json"));

    assert_required_fields(
        "why",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "finding",
            "outcome",
            "candidate_entries",
            "next",
        ],
    );
    assert!(
        !schema
            .pointer("/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required.iter().any(|field| field == "evaluation")),
        "why v1 keeps the additive evaluation metadata optional for old artifacts"
    );
    assert_eq!(
        schema
            .pointer("/properties/evaluation/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evaluation"),
        "why should expose optional evaluation metadata"
    );
    assert_eq!(
        schema
            .pointer("/$defs/evaluation/properties/result_class/enum")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["exact_scoped", "exact_after_full_fallback"]),
        "why result classes must remain bounded and stable"
    );

    assert_eq!(
        schema
            .pointer("/properties/schema_id/const")
            .and_then(Value::as_str),
        Some("cargo-allow.why.v1"),
        "why schema_id const"
    );
    assert_eq!(
        schema
            .pointer("/properties/command/const")
            .and_then(Value::as_str),
        Some("why"),
        "why command const"
    );
    assert_eq!(
        schema
            .pointer("/properties/finding/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/current_finding"),
        "why finding should reuse current_finding"
    );
    assert_eq!(
        schema
            .pointer("/properties/outcome/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/match_outcome"),
        "why outcome should reuse match_outcome"
    );
    assert_enum_equals(
        "why match status",
        &schema,
        "/$defs/match_status/enum",
        &match_status_enum(),
    );

    let candidate = required_schema_pointer("why", &schema, "/$defs/candidate_entry");
    assert_required_fields(
        "why candidate_entry",
        candidate,
        &[
            "id",
            "kind",
            "path",
            "glob",
            "selector_glob",
            "mismatch_reasons",
        ],
    );
    let next = required_schema_pointer("why", &schema, "/properties/next");
    assert_required_fields(
        "why next",
        next,
        &["suggested_actions", "proof_commands", "proof_plans"],
    );
    let proof_plan = required_schema_pointer("why", &schema, "/$defs/proof_plan");
    assert_required_fields("why proof_plan", proof_plan, &["program", "args"]);
}
