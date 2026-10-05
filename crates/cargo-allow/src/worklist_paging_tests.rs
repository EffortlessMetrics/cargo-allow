use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;

/// Build a ranked queue of four `new_unreceipted_finding` items in distinct
/// paths, exactly as `cmd_worklist` ranks it: filter, sort, renumber.
fn ranked_queue() -> Vec<WorkItem> {
    let cfg = AllowConfig::empty();
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/a.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/b.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/c.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/d.rs",
            "method_call",
        ),
    ];
    let outcomes = vec![
        test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap a",
        ),
        test_outcome(
            MatchStatus::New,
            None,
            Some(1),
            "unreceipted panic.unwrap b",
        ),
        test_outcome(
            MatchStatus::New,
            None,
            Some(2),
            "unreceipted panic.unwrap c",
        ),
        test_outcome(
            MatchStatus::New,
            None,
            Some(3),
            "unreceipted panic.unwrap d",
        ),
    ];

    let mut items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    sort_work_items(&mut items);
    renumber_work_items(&mut items);
    items
}

fn paged_context(
    filters: WorklistFilters<'static>,
    paging: WorklistPaging,
) -> WorklistContext<'static> {
    WorklistContext {
        filters,
        paging,
        ..WorklistContext::default()
    }
}

#[test]
fn worklist_paging_is_applied_tracks_limit_and_offset() {
    assert!(!WorklistPaging::default().is_applied());
    assert!(
        WorklistPaging {
            limit: Some(1),
            ..WorklistPaging::default()
        }
        .is_applied()
    );
    assert!(
        WorklistPaging {
            offset: 1,
            ..WorklistPaging::default()
        }
        .is_applied()
    );
    assert!(
        !WorklistPaging {
            limit: None,
            offset: 0,
            total: 4,
            unfiltered_total: 4,
        }
        .is_applied()
    );
}

#[test]
fn worklist_paging_composes_with_filters_after_ranking() {
    let cfg = AllowConfig::empty();
    let findings = vec![
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/a.rs",
            "method_call",
        ),
        test_finding(
            FindingKind::Unsafe,
            Some("block"),
            "src/ffi.rs",
            "unsafe_block",
        ),
        test_finding(
            FindingKind::Unsafe,
            Some("block"),
            "src/ffi_two.rs",
            "unsafe_block",
        ),
    ];
    let outcomes = vec![
        test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap a",
        ),
        test_outcome(MatchStatus::New, None, Some(1), "unreceipted unsafe block"),
        test_outcome(MatchStatus::New, None, Some(2), "unreceipted unsafe block"),
    ];

    let unfiltered_total = findings.len();
    let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
    let mut items = filter_work_items(
        items,
        WorklistFilters {
            kind: Some("unsafe"),
            ..WorklistFilters::default()
        },
    );
    assert_eq!(items.len(), 2, "kind filter narrows to the unsafe items");
    sort_work_items(&mut items);
    renumber_work_items(&mut items);

    let paging = WorklistPaging {
        limit: Some(1),
        offset: 1,
        total: items.len(),
        unfiltered_total,
    };
    let page = page_work_items(items, paging);

    assert_eq!(page.len(), 1);
    let item = page
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected the second filtered item"));
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.path.as_deref(), Some("src/ffi_two.rs"));
    // Renumbering runs before paging, so the emitted slice keeps the
    // filtered-queue item ID.
    assert_eq!(item.id, "work-new-unreceipted-finding-0002");

    let json =
        render_worklist_json_with_context(&page, paged_context(WorklistFilters::default(), paging));
    assert!(json.contains("\"paging\""));
    assert!(json.contains("\"limit\": 1"));
    assert!(json.contains("\"offset\": 1"));
    assert!(json.contains("\"total\": 2"));
    assert!(json.contains("\"unfiltered_total\": 3"));
    assert!(json.contains("\"work_items\": 1"));
}

#[test]
fn worklist_limit_and_offset_slice_the_ranked_queue() {
    let items = ranked_queue();
    let ranked_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();

    let paging = WorklistPaging {
        limit: Some(2),
        offset: 1,
        total: items.len(),
        unfiltered_total: items.len(),
    };
    let page = page_work_items(items, paging);

    assert_eq!(
        page.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
        ranked_ids[1..3],
        "offset skips the first ranked item and limit takes the next two"
    );
}

#[test]
fn worklist_limit_beyond_queue_size_emits_whole_queue() {
    let items = ranked_queue();

    let paging = WorklistPaging {
        limit: Some(100),
        offset: 0,
        total: items.len(),
        unfiltered_total: items.len(),
    };
    let page = page_work_items(items, paging);

    assert_eq!(page.len(), 4);
    assert_eq!(
        page.first().map(|item| item.id.as_str()),
        Some("work-new-unreceipted-finding-0001")
    );
    assert_eq!(
        page.last().map(|item| item.id.as_str()),
        Some("work-new-unreceipted-finding-0004")
    );

    let json =
        render_worklist_json_with_context(&page, paged_context(WorklistFilters::default(), paging));
    assert!(json.contains("\"limit\": 100"));
    assert!(json.contains("\"offset\": 0"));
    assert!(json.contains("\"total\": 4"));
    assert!(json.contains("\"unfiltered_total\": 4"));
    assert!(json.contains("\"work_items\": 4"));
}

#[test]
fn worklist_offset_beyond_queue_size_emits_described_empty_page() {
    let items = ranked_queue();

    let paging = WorklistPaging {
        limit: None,
        offset: 10,
        total: items.len(),
        unfiltered_total: items.len(),
    };
    let page = page_work_items(items, paging);

    assert!(page.is_empty());
    // An empty page keeps its paging block so the artifact records that the
    // queue was paged rather than claiming the repository is clean.
    let json =
        render_worklist_json_with_context(&page, paged_context(WorklistFilters::default(), paging));
    assert!(json.contains("\"paging\""));
    assert!(json.contains("\"limit\": null"));
    assert!(json.contains("\"offset\": 10"));
    assert!(json.contains("\"total\": 4"));
    assert!(json.contains("\"work_items\": 0"));
    assert!(
        json.contains("\"work_items\": [\n\n  ]"),
        "an empty page emits an empty work_items array"
    );
}

#[test]
fn worklist_default_render_keeps_unpaged_artifact_shape() {
    let items = ranked_queue();

    let json = render_worklist_json_with_context(
        &items,
        paged_context(WorklistFilters::default(), WorklistPaging::default()),
    );

    assert!(
        !json.contains("\"paging\""),
        "the default artifact must keep the unpaged worklist.v1 shape"
    );
    assert!(json.contains("\"filters\""));
    assert!(json.contains("\"summary\""));
    assert!(json.contains("\"work_items\": 4"));
    assert_eq!(
        json.matches("work-new-unreceipted-finding-").count(),
        4,
        "the default queue emits every ranked item"
    );
}

#[test]
fn worklist_stale_items_page_behind_new_findings_with_limit() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::NonRustFile));
    let findings = vec![test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "src/a.rs",
        "method_call",
    )];
    let outcomes = vec![
        test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap a",
        ),
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

    let paging = WorklistPaging {
        limit: Some(1),
        offset: 1,
        total: items.len(),
        unfiltered_total: items.len(),
    };
    let page = page_work_items(items, paging);

    assert_eq!(page.len(), 1);
    let item = page
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected the stale item on page two"));
    assert_eq!(item.kind, "stale_allow");
    assert_eq!(item.id, "work-stale-allow-0002");
}
