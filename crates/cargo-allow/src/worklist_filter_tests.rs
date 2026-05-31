use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;

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
fn worklist_filters_by_exception_kind() {
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Unsafe,
            Some("block"),
            "src/ffi.rs",
            "unsafe_block",
        ),
    ];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted unsafe block"),
    ];

    let items = work_items_from_outcomes(&AllowConfig::empty(), &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            kind: Some("unsafe"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.path.as_deref(), Some("src/ffi.rs"));
}

#[test]
fn worklist_filters_by_exception_kind_alias() {
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::NonRustFile,
            Some("shell_script"),
            "scripts/release.sh",
            "tracked_file",
        ),
    ];
    let outcomes = vec![
        test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted shell script"),
    ];

    let items = work_items_from_outcomes(&AllowConfig::empty(), &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            kind: Some("non-rust"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.path.as_deref(), Some("scripts/release.sh"));
}

#[test]
fn worklist_filters_by_policy_exception_family_alias() {
    let findings = vec![
        test_finding(
            FindingKind::PolicyException,
            Some("workflow_external_action"),
            ".github/workflows/ci.yml",
            "github_action_uses",
        ),
        test_finding(
            FindingKind::PolicyException,
            Some("process_spawn"),
            ".github/workflows/ci.yml",
            "process_spawn",
        ),
    ];
    let outcomes = vec![
        test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted workflow action",
        ),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted process spawn"),
    ];

    let items = work_items_from_outcomes(&AllowConfig::empty(), &findings, &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            kind: Some("workflow"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
    assert_eq!(item.family.as_deref(), Some("workflow_external_action"));
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
