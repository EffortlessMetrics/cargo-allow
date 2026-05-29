use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;

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
