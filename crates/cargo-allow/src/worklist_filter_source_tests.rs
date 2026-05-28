use super::test_support::{test_finding, test_outcome};
use super::*;

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
