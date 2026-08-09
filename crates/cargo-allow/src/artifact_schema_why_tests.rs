use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, match_status_enum, parse_schema,
    required_schema_pointer,
};
use serde_json::{Value, json};

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
        Some(vec![
            "exact_scoped",
            "exact_after_full_fallback",
            "target_scanner_partial",
            "full_fallback_unavailable",
        ]),
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
    for (field, reference) in [
        ("finding", "#/$defs/current_finding"),
        ("outcome", "#/$defs/match_outcome"),
    ] {
        let any_of = schema
            .pointer(&format!("/properties/{field}/anyOf"))
            .and_then(Value::as_array)
            .expect("nullable why fields should use anyOf");
        assert!(
            any_of
                .iter()
                .any(|variant| { variant.get("$ref").and_then(Value::as_str) == Some(reference) }),
            "why {field} should reuse {reference}"
        );
        assert!(
            any_of
                .iter()
                .any(|variant| variant.get("type") == Some(&json!("null"))),
            "why {field} should permit null for partial target scans"
        );
    }
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

#[test]
fn result_class_schema_binds_to_its_evidence_tuple() -> Result<(), String> {
    let cases = [
        (
            "why",
            include_str!("../../../docs/schemas/why.schema.json"),
            crate::why::sample_why_json_for_contract_test(),
        ),
        (
            "add-finding-plan",
            include_str!("../../../docs/schemas/add-finding-plan.schema.json"),
            crate::why::sample_add_finding_plan_json_for_contract_test(),
        ),
    ];

    for (name, schema_text, sample_text) in cases {
        let schema = parse_schema(name, schema_text);
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| format!("{name} schema compilation: {error}"))?;
        let mut partial: Value = serde_json::from_str(&sample_text)
            .map_err(|error| format!("{name} sample JSON: {error}"))?;
        partial
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain evaluation"))?
            .extend([
                ("result_class".to_string(), json!("target_scanner_partial")),
                ("scope".to_string(), json!("scoped")),
                ("locality".to_string(), json!("proven")),
                ("reasons".to_string(), json!([])),
                ("scanner_completeness".to_string(), json!("partial")),
            ]);
        partial
            .get_mut("inventory")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain inventory"))?
            .insert("completeness".to_string(), json!("partial"));
        if validator.validate(&partial).is_err() {
            return Err(format!(
                "{name} schema should accept target_scanner_partial"
            ));
        }

        let mut scoped_partial: Value = serde_json::from_str(&sample_text)
            .map_err(|error| format!("{name} sample JSON: {error}"))?;
        scoped_partial
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain evaluation"))?
            .extend([
                ("result_class".to_string(), json!("exact_scoped")),
                ("scope".to_string(), json!("scoped")),
                ("locality".to_string(), json!("proven")),
                ("reasons".to_string(), json!([])),
                ("scanner_completeness".to_string(), json!("complete")),
            ]);
        scoped_partial
            .get_mut("inventory")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain inventory"))?
            .insert("completeness".to_string(), json!("partial"));
        if validator.validate(&scoped_partial).is_err() {
            return Err(format!(
                "{name} schema should accept exact_scoped with complete target scanner evidence"
            ));
        }

        scoped_partial
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain evaluation"))?
            .remove("scanner_completeness");
        if validator.validate(&scoped_partial).is_ok() {
            return Err(format!(
                "{name} schema must require scanner evidence for exact_scoped partial inventory"
            ));
        }

        let mut unavailable: Value = serde_json::from_str(&sample_text)
            .map_err(|error| format!("{name} sample JSON: {error}"))?;
        unavailable
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain evaluation"))?
            .extend([
                (
                    "result_class".to_string(),
                    json!("full_fallback_unavailable"),
                ),
                ("scope".to_string(), json!("full_fallback")),
                ("locality".to_string(), json!("global_dependency")),
                ("reasons".to_string(), json!(["scanner incomplete"])),
                ("scanner_completeness".to_string(), json!("partial")),
            ]);
        unavailable
            .get_mut("inventory")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain inventory"))?
            .insert("completeness".to_string(), json!("fallback"));
        if validator.validate(&unavailable).is_err() {
            return Err(format!(
                "{name} schema should accept full_fallback_unavailable"
            ));
        }

        let mut contradictory: Value = serde_json::from_str(&sample_text)
            .map_err(|error| format!("{name} sample JSON: {error}"))?;
        let evaluation = contradictory
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain evaluation"))?;
        evaluation.insert("result_class".to_string(), json!("exact_scoped"));
        evaluation.insert("scope".to_string(), json!("full_fallback"));
        evaluation.insert("locality".to_string(), json!("global_dependency"));
        evaluation.insert("reasons".to_string(), json!(["requires fallback"]));
        if validator.validate(&contradictory).is_ok() {
            return Err(format!(
                "{name} schema must reject exact_scoped with full fallback evidence"
            ));
        }

        let mut incomplete: Value = serde_json::from_str(&sample_text)
            .map_err(|error| format!("{name} sample JSON: {error}"))?;
        incomplete
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain evaluation"))?
            .extend([
                (
                    "result_class".to_string(),
                    json!("exact_after_full_fallback"),
                ),
                ("scope".to_string(), json!("full_fallback")),
                ("locality".to_string(), json!("global_dependency")),
                ("reasons".to_string(), json!(["scanner incomplete"])),
            ]);
        incomplete
            .get_mut("inventory")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("{name} sample should contain inventory"))?
            .insert("completeness".to_string(), json!("partial"));
        if validator.validate(&incomplete).is_ok() {
            return Err(format!(
                "{name} schema must reject exact full-fallback classes with partial inventory"
            ));
        }
    }

    Ok(())
}
