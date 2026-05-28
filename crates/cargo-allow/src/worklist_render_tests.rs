use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;
use allow_core::{AllowConfig, FindingKind, MatchStatus};

#[test]
fn worklist_json_emits_stale_allow_actions() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
    entry.lifecycle.created = Some("2026-05-01".to_string());
    entry.lifecycle.review_after = Some("2026-06-01".to_string());
    entry.lifecycle.expires = Some("2026-08-01".to_string());
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
    assert!(json.contains("\"review_after\": \"2026-06-01\""));
    assert!(json.contains("\"expires\": \"2026-08-01\""));
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
    assert!(human.contains("review_after: 2026-06-01"));
    assert!(human.contains("expires: 2026-08-01"));
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
