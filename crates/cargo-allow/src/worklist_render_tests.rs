use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{AllowConfig, FindingKind, MatchStatus};
use clap::Parser;
use std::path::Path;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_worklist_json_output() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "worklist",
        "--kind",
        "unsafe",
        "--family",
        "unsafe_fn",
        "--item-kind",
        "baseline_debt",
        "--status",
        "baseline_debt",
        "--allow-id",
        "allow-0001",
        "--path",
        "crates/allow-core",
        "--source-package",
        "allow-core",
        "--owner",
        "runtime",
        "--classification",
        "baseline_debt",
        "--baseline-debt",
        "--broad-scope",
        "--risk",
        "medium",
        "--difficulty",
        "small",
        "--missing-evidence",
        "--format",
        "json",
        "--output",
        "target/worklist.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse worklist args: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Worklist(WorklistArgs {
            kind: Some(kind),
            family: Some(family),
            item_kind: Some(item_kind),
            status: Some(status),
            allow_id: Some(allow_id),
            path: Some(path_filter),
            source_package: Some(source_package),
            owner: Some(owner),
            classification: Some(classification),
            baseline_debt: true,
            broad_scope: true,
            risk: Some(risk),
            difficulty: Some(difficulty),
            missing_evidence: true,
            format: WorklistFormat::Json,
            output: Some(path),
            ..
        })) if kind == "unsafe"
            && family == "unsafe_fn"
            && item_kind == "baseline_debt"
            && status == "baseline_debt"
            && allow_id == "allow-0001"
            && path_filter == "crates/allow-core"
            && source_package == "allow-core"
            && owner == "runtime"
            && classification == "baseline_debt"
            && risk == "medium"
            && difficulty == "small"
            && path == Path::new("target/worklist.json")
    ));
}

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
fn worklist_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/worklist.schema.json");

    assert!(schema.contains(allow_report::WORKLIST_SCHEMA_ID));
    assert!(schema.contains("\"exception_kind\""));
    assert!(schema.contains("\"family\""));
    assert!(schema.contains("\"owner\""));
    assert!(schema.contains("\"classification\""));
    assert!(schema.contains("\"reason\""));
    assert!(schema.contains("\"created\""));
    assert!(schema.contains("\"review_after\""));
    assert!(schema.contains("\"expires\""));
    assert!(schema.contains("\"evidence_count\""));
    assert!(schema.contains("\"source_package\""));
    assert!(schema.contains("\"proof_commands\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"macro_expansion_not_analyzed\""));
    assert!(schema.contains("\"small_difficulty\""));
    assert!(schema.contains("\"medium_difficulty\""));
    assert!(schema.contains("\"filters\""));
    assert!(schema.contains("\"family\""));
    assert!(schema.contains("\"item_kind\""));
    assert!(schema.contains("\"status\""));
    assert!(schema.contains("\"allow_id\""));
    assert!(schema.contains("\"path\""));
    assert!(schema.contains("\"source_package\""));
    assert!(schema.contains("\"baseline_debt\""));
    assert!(schema.contains("\"broad_scope\""));
    assert!(schema.contains("\"missing_evidence\""));
    assert!(schema.contains("\"inventory\""));
    assert!(schema.contains("\"git_tracked\""));
    assert!(schema.contains("\"source_tree_inventory\""));
}

#[test]
fn worklist_renderers_include_inventory_context() {
    let items = Vec::new();
    let context = WorklistContext {
        inventory_source: "git_tracked",
        source_tree_root: Some("H:/Code/Rust/cargo-allow"),
        inventory_files: Some(46),
        filters: WorklistFilters::default(),
    };

    let json = render_worklist_json_with_context(&items, context);
    let human = render_worklist_human_with_context(&items, context);

    assert!(json.contains("\"scope\": \"source_tree\""));
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 46"));
    assert!(json.contains("\"filters\""));
    assert!(json.contains("\"risk\": null"));
    assert!(
        human.contains("Inventory: source_tree/source_syntax via git_tracked; files scanned: 46")
    );
    assert!(human.contains("Source tree root: H:/Code/Rust/cargo-allow"));
    assert!(human.contains("Filters: none"));
}

#[test]
fn worklist_renderers_include_applied_filters() {
    let items = Vec::new();
    let context = WorklistContext {
        inventory_source: "git_tracked",
        source_tree_root: None,
        inventory_files: Some(46),
        filters: WorklistFilters {
            kind: Some("unsafe"),
            family: Some("unsafe_fn"),
            item_kind: Some("baseline_debt"),
            status: Some("baseline_debt"),
            allow_id: Some("allow-0001"),
            path: Some("crates/allow-core"),
            source_package: Some("allow-core"),
            owner: Some("runtime"),
            classification: Some("baseline_debt"),
            baseline_debt: true,
            broad_scope: true,
            risk: Some("high"),
            difficulty: Some("medium"),
            missing_evidence: true,
        },
    };

    let json = render_worklist_json_with_context(&items, context);
    let human = render_worklist_human_with_context(&items, context);

    assert!(json.contains("\"filters\""));
    assert!(json.contains("\"kind\": \"unsafe\""));
    assert!(json.contains("\"family\": \"unsafe_fn\""));
    assert!(json.contains("\"item_kind\": \"baseline_debt\""));
    assert!(json.contains("\"status\": \"baseline_debt\""));
    assert!(json.contains("\"allow_id\": \"allow-0001\""));
    assert!(json.contains("\"path\": \"crates/allow-core\""));
    assert!(json.contains("\"source_package\": \"allow-core\""));
    assert!(json.contains("\"owner\": \"runtime\""));
    assert!(json.contains("\"classification\": \"baseline_debt\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"broad_scope\": true"));
    assert!(json.contains("\"risk\": \"high\""));
    assert!(json.contains("\"difficulty\": \"medium\""));
    assert!(json.contains("\"missing_evidence\": true"));
    assert!(human.contains(
            "Filters: kind=unsafe, family=unsafe_fn, item_kind=baseline_debt, status=baseline_debt, allow_id=allow-0001, path=crates/allow-core, source_package=allow-core, owner=runtime, classification=baseline_debt, baseline_debt=true, broad_scope=true, risk=high, difficulty=medium, missing_evidence=true"
        ));
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
