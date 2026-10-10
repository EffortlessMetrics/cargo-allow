use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;
use allow_core::{AllowConfig, FindingKind, MatchStatus};
use serde_json::{Value, json};

#[test]
fn rendered_worklist_locations_validate_against_schema() -> Result<(), String> {
    let schema: Value =
        serde_json::from_str(include_str!("../../../docs/schemas/worklist.schema.json"))
            .map_err(|error| format!("worklist schema JSON: {error}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("worklist schema compilation: {error}"))?;
    let cfg = AllowConfig::empty();

    // Run the real outcome-to-work-item projection and renderer at both
    // one-based location bounds and with no span. None must omit the fields.
    for coordinate in [Some(1), Some(u32::MAX), None] {
        let mut finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        );
        finding.span = coordinate.map(|coordinate| allow_core::Span {
            line: coordinate,
            column: coordinate,
        });
        let outcomes = [test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap in src/lib.rs",
        )];
        let items = work_items_from_outcomes(&cfg, &[finding], &outcomes);
        if items.len() != 1 {
            return Err("worklist fixture must produce one source-backed item".to_string());
        }
        let rendered = render_worklist_json_with_context(&items, WorklistContext::default());
        let artifact: Value = serde_json::from_str(&rendered)
            .map_err(|error| format!("rendered worklist JSON: {error}"))?;
        let row = artifact
            .pointer("/work_items/0")
            .and_then(Value::as_object)
            .ok_or_else(|| "rendered worklist must contain its source-backed item".to_string())?;
        for field in ["line", "column"] {
            match (coordinate, row.get(field)) {
                (Some(expected), Some(value)) if value.as_u64() == Some(u64::from(expected)) => {}
                (None, None) => {}
                _ => return Err(format!("worklist {field} did not preserve {coordinate:?}")),
            }
        }
        validator
            .validate(&artifact)
            .map_err(|error| format!("rendered worklist violates schema: {error}"))?;

        for field in ["line", "column"] {
            for invalid in [
                json!(0),
                json!(-1),
                json!(1.5),
                json!("1"),
                Value::Null,
                json!(u64::from(u32::MAX) + 1),
            ] {
                let mut invalid_location = artifact.clone();
                invalid_location
                    .pointer_mut("/work_items/0")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| "missing worklist negative-control row".to_string())?
                    .insert(field.to_string(), invalid.clone());
                if validator.validate(&invalid_location).is_ok() {
                    return Err(format!("worklist schema must reject {field} {invalid}"));
                }
            }
        }
        let mut unknown_field = artifact.clone();
        unknown_field
            .pointer_mut("/work_items/0")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "missing worklist negative-control row".to_string())?
            .insert("unexpected_field".to_string(), json!(true));
        if validator.validate(&unknown_field).is_ok() {
            return Err("worklist schema must reject undeclared item fields".to_string());
        }
    }
    Ok(())
}

#[test]
fn worklist_json_emits_stale_allow_actions() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
    entry.lifecycle.created = Some("2026-05-01".to_string());
    entry.lifecycle.review_after = Some("2030-06-01".to_string());
    entry.lifecycle.expires = Some("2030-08-01".to_string());
    entry.evidence = vec!["doc:docs/policy/file.md".to_string()];
    cfg.allow.push(entry);
    let outcomes = vec![test_outcome(
        MatchStatus::Stale,
        Some("allow-file"),
        None,
        "allow-file is stale: no current finding matched tracked.file",
    )];

    let items = work_items_from_outcomes(&cfg, &[], &outcomes);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    assert_eq!(items.len(), 1);
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::WORKLIST_SCHEMA_ID
    )));
    assert!(json.contains("\"source_tree_inventory\""));
    assert!(json.contains("\"cargo_commands_not_invoked\""));
    assert!(json.contains("\"repository_code_not_executed\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"inventory\""));
    assert!(json.contains("\"source\": \"unknown\""));
    assert!(json.contains("\"kind\": \"stale_allow\""));
    assert!(json.contains("\"exception_kind\": \"non_rust_file\""));
    assert!(json.contains("\"family\": null"));
    assert!(json.contains("\"owner\": \"owner\""));
    assert!(json.contains("\"classification\": \"classification\""));
    assert!(json.contains("\"reason\": \"reason\""));
    assert!(json.contains("\"created\": \"2026-05-01\""));
    assert!(json.contains("\"review_after\": \"2030-06-01\""));
    assert!(json.contains("\"expires\": \"2030-08-01\""));
    assert!(json.contains("\"evidence_count\": 1"));
    assert!(json.contains("\"risk\": \"low\""));
    assert!(json.contains("\"small_difficulty\": 1"));
    assert!(json.contains("\"medium_difficulty\": 0"));
    assert!(json.contains("\"source_package\": null"));
    assert!(json.contains("\"cargo-allow explain allow-file\""));
    assert!(json.contains("\"cargo-allow check --kind non-rust --mode no-new\""));
    assert!(human.contains("owner: owner"));
    assert!(human.contains("classification: classification"));
    assert!(human.contains("reason: reason"));
    assert!(human.contains("created: 2026-05-01"));
    assert!(human.contains("review_after: 2030-06-01"));
    assert!(human.contains("expires: 2030-08-01"));
    assert!(human.contains("evidence: 1 reference(s)"));
}

#[test]
fn worklist_human_output_reports_truncated_items() {
    let cfg = AllowConfig::empty();
    let findings = (0..81)
        .map(|index| {
            test_finding(
                FindingKind::Panic,
                Some("unwrap"),
                &format!("src/file_{index}.rs"),
                "method_call",
            )
        })
        .collect::<Vec<_>>();
    let outcomes = (0..81)
        .map(|index| {
            test_outcome(
                MatchStatus::New,
                None,
                Some(index),
                &format!("unreceipted panic.unwrap at src/file_{index}.rs:1:1"),
            )
        })
        .collect::<Vec<_>>();

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    assert!(human.contains("work-new-unreceipted-finding-0080"));
    assert!(!human.contains("work-new-unreceipted-finding-0081"));
    assert!(human.contains("1 additional work items omitted from human output"));
    assert!(human.contains("cargo-allow worklist --format json"));
}
