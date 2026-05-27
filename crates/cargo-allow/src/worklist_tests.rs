use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{AllowEntry, Lifecycle, Selector, Span, StructuralIdentity};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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

#[test]
fn worklist_items_prioritize_unsafe_new_findings() {
    let cfg = AllowConfig::empty();
    let findings = vec![test_finding(
        FindingKind::Unsafe,
        Some("unsafe_fn"),
        "src/lib.rs",
        "unsafe_fn",
    )];
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        None,
        Some(0),
        "unreceipted unsafe.unsafe_fn at src/lib.rs:1:1",
    )];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let text = render_worklist_human_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "new_unreceipted_finding");
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.family.as_deref(), Some("unsafe_fn"));
    assert_eq!(item.risk, "high");
    assert!(
        item.proof_commands
            .iter()
            .any(|command| { command == "cargo-allow check --kind unsafe --mode no-new" })
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --kind unsafe --format json")
    );
    assert!(
        item.proof_commands
            .iter()
            .all(|command| command.starts_with("cargo-allow "))
    );
    assert!(text.contains("work-new-unreceipted-finding-0001"));
    assert!(text.contains("exception: unsafe.unsafe_fn"));
    assert!(text.contains("action: remove the new source exception if it is accidental"));
    assert!(text.contains("proof: cargo-allow check --kind unsafe --mode no-new"));
    assert!(text.contains("proof: cargo-allow worklist --kind unsafe --format json"));
    assert!(text.contains("Difficulty:"));
    assert!(text.contains("  medium    1"));
}

#[test]
fn worklist_items_include_explicit_source_package_context() {
    let cfg = AllowConfig::empty();
    let mut finding = test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "crates/parser/src/lib.rs",
        "method_call",
    );
    finding.identity.crate_name = Some("parser".to_string());
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        None,
        Some(0),
        "unreceipted panic.unwrap at crates/parser/src/lib.rs:1:1",
    )];

    let items = work_items_from_outcomes(&cfg, &[finding], &outcomes);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());
    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));

    assert_eq!(item.source_package.as_deref(), Some("parser"));
    assert_eq!(item.exception_kind.as_deref(), Some("panic"));
    assert_eq!(item.family.as_deref(), Some("unwrap"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("package `parser`"))
    );
    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"exception_kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(human.contains("source package: parser"));
    assert!(human.contains("exception: panic.unwrap"));
    assert!(
        item.proof_commands
            .iter()
            .all(|command| command.starts_with("cargo-allow "))
    );
}

#[test]
fn worklist_items_prioritize_process_policy_findings() {
    let cfg = AllowConfig::empty();
    let findings = vec![test_finding(
        FindingKind::PolicyException,
        Some("process_spawn"),
        ".github/workflows/ci.yml",
        "process_spawn",
    )];
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        None,
        Some(0),
        "unreceipted policy_exception.process_spawn at .github/workflows/ci.yml:1:1",
    )];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "new_unreceipted_finding");
    assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
    assert_eq!(item.family.as_deref(), Some("process_spawn"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "medium");
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow check --kind process --mode no-new")
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --kind process --format json")
    );
}

#[test]
fn worklist_items_treat_new_non_rust_files_as_small() {
    let cfg = AllowConfig::empty();
    let findings = vec![test_finding(
        FindingKind::NonRustFile,
        Some("shell_script"),
        "scripts/new.sh",
        "tracked_file",
    )];
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        None,
        Some(0),
        "unreceipted non_rust_file.shell_script at scripts/new.sh:1:1",
    )];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "new_unreceipted_finding");
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.family.as_deref(), Some("shell_script"));
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "small");
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow check --kind non-rust --mode no-new")
    );
}

#[test]
fn worklist_items_keep_stale_allows_low_risk_even_for_unsafe() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-unsafe", FindingKind::Unsafe));
    let outcomes = vec![test_outcome(
        MatchStatus::Stale,
        Some("allow-unsafe"),
        None,
        "allow-unsafe is stale: no current finding matched src/lib.rs",
    )];

    let items = work_items_from_outcomes(&cfg, &[], &outcomes);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "stale_allow");
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.risk, "low");
    assert_eq!(item.difficulty, "small");
}

#[test]
fn worklist_filters_by_risk_and_difficulty() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::NonRustFile));
    let findings = vec![
        test_finding(
            FindingKind::PolicyException,
            Some("process_spawn"),
            ".github/workflows/ci.yml",
            "process_spawn",
        ),
        test_finding(
            FindingKind::NonRustFile,
            Some("shell_script"),
            "scripts/new.sh",
            "tracked_file",
        ),
    ];
    let outcomes = vec![
        test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted process policy exception",
        ),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted shell script"),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-stale"),
            None,
            "allow-stale is stale",
        ),
    ];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            risk: Some("medium"),
            difficulty: Some("small"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.kind, "new_unreceipted_finding");
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.path.as_deref(), Some("scripts/new.sh"));
}

#[test]
fn worklist_filters_by_owner_and_classification() {
    let mut cfg = AllowConfig::empty();
    let mut first = test_entry("allow-first", FindingKind::NonRustFile);
    first.owner = "team-a".to_string();
    first.classification = "baseline_debt".to_string();
    let mut second = test_entry("allow-second", FindingKind::NonRustFile);
    second.owner = "team-b".to_string();
    second.classification = "reviewed_exception".to_string();
    cfg.allow.push(first);
    cfg.allow.push(second);
    let outcomes = vec![
        test_outcome(
            MatchStatus::Stale,
            Some("allow-first"),
            None,
            "allow-first is stale",
        ),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-second"),
            None,
            "allow-second is stale",
        ),
    ];

    let items = work_items_from_outcomes(&cfg, &[], &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            owner: Some("team-a"),
            classification: Some("baseline_debt"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.allow_id.as_deref(), Some("allow-first"));
    assert_eq!(item.owner.as_deref(), Some("team-a"));
    assert_eq!(item.classification.as_deref(), Some("baseline_debt"));
}

#[test]
fn worklist_filters_by_item_kind() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::NonRustFile));
    let findings = vec![test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
    )];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-stale"),
            None,
            "allow-stale is stale",
        ),
    ];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            item_kind: Some("stale_allow"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.kind, "stale_allow");
    assert_eq!(item.allow_id.as_deref(), Some("allow-stale"));
}

#[test]
fn worklist_filters_by_status() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::NonRustFile));
    let findings = vec![test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
    )];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-stale"),
            None,
            "allow-stale is stale",
        ),
    ];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            status: Some("stale"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.status, MatchStatus::Stale);
    assert_eq!(item.allow_id.as_deref(), Some("allow-stale"));
}

#[test]
fn worklist_filters_by_allow_id() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-first", FindingKind::NonRustFile));
    cfg.allow
        .push(test_entry("allow-second", FindingKind::NonRustFile));
    let outcomes = vec![
        test_outcome(
            MatchStatus::Stale,
            Some("allow-first"),
            None,
            "allow-first is stale",
        ),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-second"),
            None,
            "allow-second is stale",
        ),
    ];

    let items = work_items_from_outcomes(&cfg, &[], &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            allow_id: Some("allow-second"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.allow_id.as_deref(), Some("allow-second"));
}

#[test]
fn worklist_filters_by_advisory_shortcuts() {
    let baseline = WorkItem {
        id: "work-baseline-debt-0001".to_string(),
        kind: "baseline_debt".to_string(),
        exception_kind: Some("panic".to_string()),
        family: Some("unwrap".to_string()),
        owner: Some("runtime".to_string()),
        classification: Some("baseline_debt".to_string()),
        reason: Some("fixture".to_string()),
        created: None,
        review_after: None,
        expires: Some("2026-08-01".to_string()),
        evidence_count: Some(0),
        risk: "medium",
        difficulty: "medium",
        status: MatchStatus::BaselineDebt,
        allow_id: Some("allow-baseline".to_string()),
        finding_index: None,
        path: Some("src/lib.rs".to_string()),
        source_package: None,
        message: "baseline debt".to_string(),
        suggested_actions: Vec::new(),
        proof_commands: Vec::new(),
    };
    let mut broad = baseline.clone();
    broad.id = "work-broad-scope-0002".to_string();
    broad.kind = "broad_scope".to_string();
    broad.classification = Some("reviewed_exception".to_string());
    broad.status = MatchStatus::Matched;
    broad.allow_id = Some("allow-broad".to_string());
    let mut stale = broad.clone();
    stale.id = "work-stale-0003".to_string();
    stale.kind = "stale_allow".to_string();
    stale.status = MatchStatus::Stale;
    stale.allow_id = Some("allow-stale".to_string());

    let baseline_filtered = filter_work_items(
        vec![baseline.clone(), broad.clone(), stale.clone()],
        WorklistFilters {
            baseline_debt: true,
            ..WorklistFilters::default()
        },
    );
    let broad_filtered = filter_work_items(
        vec![baseline, broad, stale],
        WorklistFilters {
            broad_scope: true,
            ..WorklistFilters::default()
        },
    );

    assert_eq!(baseline_filtered.len(), 1);
    assert_eq!(
        baseline_filtered[0].allow_id.as_deref(),
        Some("allow-baseline")
    );
    assert_eq!(broad_filtered.len(), 1);
    assert_eq!(broad_filtered[0].allow_id.as_deref(), Some("allow-broad"));
}

#[test]
fn worklist_filters_by_missing_evidence() {
    let missing = WorkItem {
        id: "work-missing-evidence-0001".to_string(),
        kind: "missing_evidence".to_string(),
        exception_kind: Some("unsafe".to_string()),
        family: Some("unsafe_block".to_string()),
        owner: Some("runtime".to_string()),
        classification: Some("reviewed_unsafe_boundary".to_string()),
        reason: Some("fixture".to_string()),
        created: None,
        review_after: None,
        expires: None,
        evidence_count: Some(0),
        risk: "high",
        difficulty: "small",
        status: MatchStatus::EvidenceMissing,
        allow_id: Some("allow-missing".to_string()),
        finding_index: None,
        path: Some("src/lib.rs".to_string()),
        source_package: None,
        message: "allow-missing requires evidence".to_string(),
        suggested_actions: Vec::new(),
        proof_commands: Vec::new(),
    };
    let mut evidenced = missing.clone();
    evidenced.id = "work-review-due-0002".to_string();
    evidenced.kind = "review_due".to_string();
    evidenced.evidence_count = Some(2);
    evidenced.status = MatchStatus::ReviewDue;
    evidenced.allow_id = Some("allow-evidenced".to_string());
    let mut new_finding = missing.clone();
    new_finding.id = "work-new-unreceipted-finding-0003".to_string();
    new_finding.kind = "new_unreceipted_finding".to_string();
    new_finding.evidence_count = None;
    new_finding.status = MatchStatus::New;
    new_finding.allow_id = None;

    let filtered = filter_work_items(
        vec![missing, evidenced, new_finding],
        WorklistFilters {
            missing_evidence: true,
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected missing evidence work item"));
    assert_eq!(item.allow_id.as_deref(), Some("allow-missing"));
    assert_eq!(item.evidence_count, Some(0));
}

#[test]
fn worklist_filters_by_path_prefix() {
    let cfg = AllowConfig::empty();
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "crates/allow-core/src/lib.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Panic,
            Some("expect"),
            "crates/allow-rust/src/lib.rs",
            "method_call",
        ),
    ];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted unwrap"),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted expect"),
    ];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            path: Some(r"crates\allow-core"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.path.as_deref(), Some("crates/allow-core/src/lib.rs"));
}

#[test]
fn worklist_filters_by_source_package() {
    let cfg = AllowConfig::empty();
    let mut first = test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "crates/allow-core/src/lib.rs",
        "method_call",
    );
    first.identity.crate_name = Some("allow-core".to_string());
    let mut second = test_finding(
        FindingKind::Panic,
        Some("expect"),
        "crates/allow-rust/src/lib.rs",
        "method_call",
    );
    second.identity.crate_name = Some("allow-rust".to_string());
    let findings = vec![first, second];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted unwrap"),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted expect"),
    ];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            source_package: Some("allow-core"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.source_package.as_deref(), Some("allow-core"));
    assert_eq!(item.path.as_deref(), Some("crates/allow-core/src/lib.rs"));
}

#[test]
fn worklist_filters_by_family() {
    let cfg = AllowConfig::empty();
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/unwrap.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Panic,
            Some("expect"),
            "src/expect.rs",
            "method_call",
        ),
    ];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted unwrap"),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted expect"),
    ];

    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            family: Some("unwrap"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.family.as_deref(), Some("unwrap"));
    assert_eq!(item.path.as_deref(), Some("src/unwrap.rs"));
}

#[test]
fn worklist_sort_prioritizes_risk_then_difficulty() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::NonRustFile));
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/panic.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::PolicyException,
            Some("process_spawn"),
            ".github/workflows/ci.yml",
            "process_spawn",
        ),
        test_finding(
            FindingKind::NonRustFile,
            Some("shell_script"),
            "scripts/new.sh",
            "tracked_file",
        ),
    ];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
        test_outcome(
            MatchStatus::New,
            None,
            Some(1),
            "unreceipted process policy exception",
        ),
        test_outcome(MatchStatus::New, None, Some(2), "unreceipted shell script"),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-stale"),
            None,
            "allow-stale is stale",
        ),
    ];

    let mut items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    sort_work_items(&mut items);
    renumber_work_items(&mut items);

    assert_eq!(items[0].risk, "high");
    assert_eq!(items[0].family.as_deref(), Some("process_spawn"));
    assert_eq!(items[0].id, "work-new-unreceipted-finding-0001");
    assert_eq!(items[1].risk, "medium");
    assert_eq!(items[1].difficulty, "small");
    assert_eq!(items[1].family.as_deref(), Some("shell_script"));
    assert_eq!(items[1].id, "work-new-unreceipted-finding-0002");
    assert_eq!(items[2].risk, "medium");
    assert_eq!(items[2].difficulty, "medium");
    assert_eq!(items[2].family.as_deref(), Some("unwrap"));
    assert_eq!(items[2].id, "work-new-unreceipted-finding-0003");
    assert_eq!(items[3].risk, "low");
    assert_eq!(items[3].kind, "stale_allow");
    assert_eq!(items[3].id, "work-stale-allow-0004");
}

#[test]
fn worklist_items_report_occurrence_limit_overrun() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-file", FindingKind::NonRustFile));
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        Some("allow-file"),
        Some(0),
        "allow-file occurrence_limit exceeded at tracked.file:1:1",
    )];

    let items = work_items_from_outcomes(&cfg, &[finding], &outcomes);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "occurrence_limit_exceeded");
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.risk, "medium");
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("baseline count"))
    );
}

#[test]
fn worklist_items_report_broad_scope_advisories() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-scripts", FindingKind::NonRustFile);
    entry.path = None;
    entry.glob = Some("scripts/**".to_string());
    entry.selector.glob = Some("scripts/**".to_string());
    entry.family = Some("shell_script".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-scripts".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broad_scope");
    assert_eq!(item.status, MatchStatus::Matched);
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.allow_id.as_deref(), Some("allow-scripts"));
    assert_eq!(item.path.as_deref(), Some("scripts/**"));
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.family.as_deref(), Some("shell_script"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("narrower glob"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --broad-scope --format json")
    );
    assert!(json.contains("\"kind\": \"broad_scope\""));
    assert!(json.contains("\"status\": \"matched\""));
    assert!(human.contains("proof: cargo-allow worklist --broad-scope --format json"));
    assert!(human.contains("exception: non_rust_file.shell_script"));
}

#[test]
fn worklist_items_report_matched_baseline_debt_advisories() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-baseline", FindingKind::Panic);
    entry.classification = "baseline_debt".to_string();
    entry.family = Some("unwrap".to_string());
    cfg.allow.push(entry);
    let mut finding = test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "crates/parser/src/lib.rs",
        "method_call",
    );
    finding.identity.crate_name = Some("parser".to_string());
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-baseline".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "baseline_debt");
    assert_eq!(item.status, MatchStatus::BaselineDebt);
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "medium");
    assert_eq!(item.allow_id.as_deref(), Some("allow-baseline"));
    assert_eq!(item.finding_index, Some(0));
    assert_eq!(item.exception_kind.as_deref(), Some("panic"));
    assert_eq!(item.family.as_deref(), Some("unwrap"));
    assert_eq!(item.source_package.as_deref(), Some("parser"));
    assert!(item.message.contains("still needs human review"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("reviewed allow entry"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --baseline-debt --format json")
    );
    assert!(json.contains("\"kind\": \"baseline_debt\""));
    assert!(json.contains("\"status\": \"baseline_debt\""));
    assert!(human.contains("proof: cargo-allow worklist --baseline-debt --format json"));
    assert!(human.contains("source package: parser"));
    assert!(human.contains("exception: panic.unwrap"));
}

#[test]
fn worklist_policy_advisories_ignore_exact_selector_globs() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-doc", FindingKind::NonRustFile);
    entry.selector.glob = Some("docs/README.md".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-doc".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    assert!(items.is_empty());
}

#[test]
fn worklist_policy_advisories_ignore_unmatched_broad_scopes() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-scripts", FindingKind::NonRustFile);
    entry.glob = Some("scripts/**".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-scripts".to_string()),
        finding_index: None,
        message: "stale".to_string(),
        score: 0,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    assert!(items.is_empty());
}

#[test]
fn worklist_items_report_broken_evidence_links() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.evidence = vec!["doc:docs/missing.md".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broken_evidence_link");
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-unsafe"));
    assert_eq!(item.path.as_deref(), Some("docs/missing.md"));
    assert!(item.message.contains("local evidence file is missing"));
    assert!(json.contains("\"kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"exception_kind\": \"unsafe\""));
    assert!(json.contains("\"cargo-allow explain allow-unsafe\""));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

static NEXT_WORKLIST_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn migrate_fixture_dir() -> PathBuf {
    let id = NEXT_WORKLIST_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-worklist-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn test_finding(kind: FindingKind, family: Option<&str>, path: &str, ast_kind: &str) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
    }
}

fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}
