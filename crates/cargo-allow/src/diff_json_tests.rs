use super::*;
use crate::diff_json_test_support::{first_array_item, parse_json, structured_diff_fixture};
use serde_json::{Value, json};

#[test]
fn rendered_diff_rows_validate_against_report_schema() -> Result<(), String> {
    let fixture = structured_diff_fixture();
    let cfg = AllowConfig::empty();
    let ledger = DiffLedgerContext::new(
        &cfg,
        &cfg,
        &fixture.finding_changes,
        &fixture.policy_changes,
        allow_report::DiffAnalysisContext::default(),
    );
    let rendered = render_diff_json_report(
        &[],
        &fixture.outcomes,
        true,
        allow_report::ReportContext::default(),
        1,
        &ledger,
    );
    let artifact: Value =
        serde_json::from_str(&rendered).map_err(|error| format!("rendered diff JSON: {error}"))?;
    let schema: Value =
        serde_json::from_str(include_str!("../../../docs/schemas/report.schema.json"))
            .map_err(|error| format!("report schema JSON: {error}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("report schema compilation: {error}"))?;

    // Empty arrays missed #4349. Exercise both real row producers, including
    // the optional policy detail objects in the existing structured fixture.
    for (field, expected_count) in [
        ("finding_changes", fixture.finding_changes.len()),
        ("policy_changes", fixture.policy_changes.len()),
    ] {
        let pointer = format!("/diff/{field}");
        let rows = artifact
            .pointer(&pointer)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("rendered diff must contain {pointer}"))?;
        if rows.is_empty() || rows.len() != expected_count {
            return Err(format!("{pointer} lost nonempty fixture rows"));
        }
        for row in rows {
            if row
                .get("coverage_movement")
                .and_then(Value::as_str)
                .is_none()
            {
                return Err(format!("{pointer} producer omitted coverage_movement"));
            }
        }
    }
    validator
        .validate(&artifact)
        .map_err(|error| format!("rendered diff violates report schema: {error}"))?;

    for field in ["finding_changes", "policy_changes"] {
        let pointer = format!("/diff/{field}/0");
        let mut unknown_field = artifact.clone();
        unknown_field
            .pointer_mut(&pointer)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("missing {pointer} negative-control row"))?
            .insert("unexpected_field".to_string(), json!(true));
        if validator.validate(&unknown_field).is_ok() {
            return Err(format!("{pointer} must reject undeclared fields"));
        }
        for invalid in [json!("unknown"), json!(0), Value::Null] {
            let mut invalid_coverage = artifact.clone();
            invalid_coverage
                .pointer_mut(&pointer)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| format!("missing {pointer} negative-control row"))?
                .insert("coverage_movement".to_string(), invalid.clone());
            if validator.validate(&invalid_coverage).is_ok() {
                return Err(format!("{pointer} must reject coverage_movement {invalid}"));
            }
        }
    }
    Ok(())
}

#[test]
fn report_schema_coverage_movement_matches_canonical_labels() -> Result<(), String> {
    let schema: Value =
        serde_json::from_str(include_str!("../../../docs/schemas/report.schema.json"))
            .map_err(|error| format!("report schema JSON: {error}"))?;
    let actual = schema
        .pointer("/$defs/coverage_movement/enum")
        .and_then(Value::as_array)
        .ok_or_else(|| "report schema must declare coverage movement labels".to_string())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "coverage movement labels must be strings".to_string())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut expected = BTreeSet::new();
    for &movement in allow_core::PresenceMovement::ALL {
        for &delta in allow_core::PostureDelta::ALL {
            for changed_in_diff in [false, true] {
                expected.insert(
                    allow_core::LedgerPosture::new(movement, delta)
                        .coverage_movement_classification(changed_in_diff),
                );
            }
        }
    }
    if actual != expected {
        return Err(format!(
            "coverage movement schema {actual:?} differs from canonical labels {expected:?}"
        ));
    }
    for definition in ["finding_posture_change", "policy_change"] {
        let pointer = format!("/$defs/{definition}/properties/coverage_movement/$ref");
        if schema.pointer(&pointer).and_then(Value::as_str) != Some("#/$defs/coverage_movement") {
            return Err(format!(
                "{definition} must reuse the coverage movement enum"
            ));
        }
    }
    Ok(())
}

#[test]
fn json_report_includes_structured_posture_changes() {
    let fixture = structured_diff_fixture();

    let cfg = allow_core::AllowConfig::empty();
    let ledger = DiffLedgerContext::new(
        &cfg,
        &cfg,
        &fixture.finding_changes,
        &fixture.policy_changes,
        allow_report::DiffAnalysisContext::default(),
    );
    let json = render_diff_json_with_posture(
        allow_report::render_json_with_context(
            "diff",
            &[],
            &[],
            false,
            allow_report::ReportContext::default(),
        ),
        1,
        &fixture.outcomes,
        &ledger,
    );
    let value = parse_json("diff report", &json);

    assert_eq!(
        value.pointer("/diff/net_posture").and_then(Value::as_str),
        Some("worse")
    );
    assert_eq!(
        value
            .pointer("/diff/reviewer_action")
            .and_then(Value::as_str),
        Some("block until failing source exception changes are fixed, narrowed, or receipted.")
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/new_findings")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(Value::as_u64),
        Some(0)
    );
    let finding_change = first_array_item(&value, "/diff/finding_changes");
    assert_eq!(
        finding_change.get("change").and_then(Value::as_str),
        Some("new")
    );
    assert_eq!(
        finding_change.get("kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        finding_change.get("family").and_then(Value::as_str),
        Some("unwrap")
    );
    assert_eq!(
        finding_change.get("path").and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert_eq!(finding_change.get("line").and_then(Value::as_u64), Some(12));
    assert_eq!(
        finding_change.get("column").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        finding_change.get("source_package").and_then(Value::as_str),
        Some("parser")
    );
    assert_eq!(
        finding_change
            .pointer("/identity/ast_kind")
            .and_then(Value::as_str),
        Some("method_call")
    );
    assert_eq!(
        finding_change
            .pointer("/identity/callee")
            .and_then(Value::as_str),
        Some("unwrap")
    );
    let policy_change = first_array_item(&value, "/diff/policy_changes");
    assert_eq!(
        policy_change.get("severity").and_then(Value::as_str),
        Some("fail")
    );
    assert_eq!(
        policy_change.get("allow_id").and_then(Value::as_str),
        Some("allow-0001")
    );
    assert_eq!(
        policy_change.get("kind").and_then(Value::as_str),
        Some("selector_precision_decreased")
    );
    assert_eq!(
        policy_change
            .pointer("/selector_precision/before")
            .and_then(Value::as_u64),
        Some(80)
    );
    assert_eq!(
        policy_change
            .pointer("/selector_precision/after")
            .and_then(Value::as_u64),
        Some(45)
    );
    assert_eq!(
        policy_change
            .pointer("/selector_precision/removed_fields/0")
            .and_then(Value::as_str),
        Some("container")
    );
    let policy_changes = value
        .pointer("/diff/policy_changes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let scope_change = policy_changes
        .get(1)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should include scope row"));
    assert_eq!(
        scope_change.get("kind").and_then(Value::as_str),
        Some("scope_broadened")
    );
    assert_eq!(
        scope_change.pointer("/scope/field").and_then(Value::as_str),
        Some("effective")
    );
    assert_eq!(
        scope_change
            .pointer("/scope/before")
            .and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert_eq!(
        scope_change.pointer("/scope/after").and_then(Value::as_str),
        Some("src/**")
    );
    let limit_change = policy_changes
        .get(2)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should include limit row"));
    assert_eq!(
        limit_change.get("kind").and_then(Value::as_str),
        Some("occurrence_limit_loosened")
    );
    assert_eq!(
        limit_change
            .pointer("/occurrence_limit/before")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        limit_change
            .pointer("/occurrence_limit/after")
            .is_some_and(Value::is_null)
    );
    let lifecycle_change = policy_changes.get(3).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include lifecycle row")
    });
    assert_eq!(
        lifecycle_change.get("kind").and_then(Value::as_str),
        Some("expiry_extended")
    );
    assert_eq!(
        lifecycle_change
            .pointer("/lifecycle/field")
            .and_then(Value::as_str),
        Some("expires")
    );
    assert_eq!(
        lifecycle_change
            .pointer("/lifecycle/before")
            .and_then(Value::as_str),
        Some("2026-09-01")
    );
    assert_eq!(
        lifecycle_change
            .pointer("/lifecycle/after")
            .and_then(Value::as_str),
        Some("2026-12-01")
    );
    let evidence_change = policy_changes.get(4).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include evidence row")
    });
    assert_eq!(
        evidence_change.get("kind").and_then(Value::as_str),
        Some("evidence_removed")
    );
    assert_eq!(
        evidence_change
            .pointer("/evidence/field")
            .and_then(Value::as_str),
        Some("evidence")
    );
    assert_eq!(
        evidence_change
            .pointer("/evidence/removed/0")
            .and_then(Value::as_str),
        Some("test:old-proof")
    );
    let added = evidence_change
        .pointer("/evidence/added")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("evidence added should be an array"));
    assert!(added.is_empty());
    let metadata_change = policy_changes.get(5).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include metadata row")
    });
    assert_eq!(
        metadata_change.get("kind").and_then(Value::as_str),
        Some("owner_removed")
    );
    assert_eq!(
        metadata_change
            .pointer("/metadata/field")
            .and_then(Value::as_str),
        Some("owner")
    );
    assert_eq!(
        metadata_change
            .pointer("/metadata/before")
            .and_then(Value::as_str),
        Some("core")
    );
    assert!(
        metadata_change
            .pointer("/metadata/after")
            .is_some_and(Value::is_null)
    );
    let requirement_change = policy_changes.get(6).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include requirement row")
    });
    assert_eq!(
        requirement_change.get("kind").and_then(Value::as_str),
        Some("requirement_loosened")
    );
    assert_eq!(
        requirement_change
            .pointer("/requirement/field")
            .and_then(Value::as_str),
        Some("owner_required")
    );
    assert!(
        requirement_change
            .pointer("/requirement/before")
            .is_some_and(|value| value == &Value::Bool(true))
    );
    assert!(
        requirement_change
            .pointer("/requirement/after")
            .is_some_and(|value| value == &Value::Bool(false))
    );
    let status_change = policy_changes.get(7).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include policy status row")
    });
    assert_eq!(
        status_change.get("kind").and_then(Value::as_str),
        Some("policy_status_weakened")
    );
    assert_eq!(
        status_change
            .pointer("/policy_status/before")
            .and_then(Value::as_str),
        Some("active")
    );
    assert_eq!(
        status_change
            .pointer("/policy_status/after")
            .and_then(Value::as_str),
        Some("advisory")
    );
    let identity_change = policy_changes.get(8).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include exception identity row")
    });
    assert_eq!(
        identity_change.get("kind").and_then(Value::as_str),
        Some("kind_changed")
    );
    assert_eq!(
        identity_change
            .pointer("/exception_identity/field")
            .and_then(Value::as_str),
        Some("kind")
    );
    assert_eq!(
        identity_change
            .pointer("/exception_identity/before")
            .and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        identity_change
            .pointer("/exception_identity/after")
            .and_then(Value::as_str),
        Some("unsafe")
    );
    let selector_identity_change = policy_changes.get(9).unwrap_or_else(|| {
        std::panic::panic_any("diff policy_changes should include selector identity row")
    });
    assert_eq!(
        selector_identity_change.get("kind").and_then(Value::as_str),
        Some("selector_changed")
    );
    assert_eq!(
        selector_identity_change
            .pointer("/selector_identity/changed_fields/0")
            .and_then(Value::as_str),
        Some("container")
    );
    assert_eq!(
        selector_identity_change
            .pointer("/selector_identity/changed_fields/1")
            .and_then(Value::as_str),
        Some("normalized_snippet_hash")
    );
    assert!(json.ends_with("}\n"));
}

#[test]
fn json_report_includes_diff_summary_evidence_health() {
    let context = allow_report::ReportContext {
        broken_evidence_links: Some(1),
        policy_missing_evidence_entries: Some(3),
        weak_evidence_references: Some(2),
        ..allow_report::ReportContext::default()
    };

    let cfg = allow_core::AllowConfig::empty();
    let ledger = DiffLedgerContext::new(
        &cfg,
        &cfg,
        &[],
        &[],
        allow_report::DiffAnalysisContext::default(),
    );
    let json = render_diff_json_report(&[], &[], true, context, 1, &ledger);
    let value = parse_json("diff report", &json);

    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/missing_evidence")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "base report summary should keep evidence health counts"
    );
    assert_eq!(
        value
            .pointer("/summary/policy_missing_evidence")
            .and_then(Value::as_u64),
        Some(3),
        "base report summary should keep policy missing evidence counts"
    );
}

#[test]
fn json_report_keeps_base_report_when_append_fails() {
    let base = "not json".to_string();

    let cfg = allow_core::AllowConfig::empty();
    let ledger = DiffLedgerContext::new(
        &cfg,
        &cfg,
        &[],
        &[],
        allow_report::DiffAnalysisContext::default(),
    );
    let json = render_diff_json_with_posture(base.clone(), 0, &[], &ledger);

    assert_eq!(json, base);
}
