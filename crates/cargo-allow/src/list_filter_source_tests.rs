use super::test_support::list_row;
use super::*;

#[test]
fn render_list_rows_filters_source_package() {
    let mut allow_core = list_row("allow-core", FindingKind::Panic, "parser", "baseline_debt");
    allow_core.source_package = Some("allow-core".to_string());
    let mut allow_rust = list_row("allow-rust", FindingKind::Panic, "scanner", "baseline_debt");
    allow_rust.source_package = Some("allow-rust".to_string());
    let rows = vec![allow_core, allow_rust];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: Some("allow-core"),
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("allow-core"));
    assert!(!text.contains("allow-rust"));
}

#[test]
fn render_list_rows_filters_family() {
    let mut indexing = list_row("allow-index", FindingKind::Panic, "parser", "baseline_debt");
    indexing.family = Some("indexing".to_string());
    let mut unwrap = list_row(
        "allow-unwrap",
        FindingKind::Panic,
        "parser",
        "baseline_debt",
    );
    unwrap.family = Some("unwrap".to_string());
    let rows = vec![indexing, unwrap];
    let filters = ListFilters {
        kind: None,
        family: Some("unwrap"),
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-index"));
    assert!(text.contains("allow-unwrap"));
}

#[test]
fn render_list_rows_filters_path_prefix_and_covering_glob() {
    let mut allow_core = list_row("allow-core", FindingKind::Panic, "parser", "baseline_debt");
    allow_core.scope = "crates/allow-core/src/lib.rs".to_string();
    let mut broad = list_row(
        "allow-broad",
        FindingKind::NonRustFile,
        "tools",
        "baseline_debt",
    );
    broad.scope = "crates/allow-core/**".to_string();
    let mut allow_rust = list_row("allow-rust", FindingKind::Panic, "scanner", "baseline_debt");
    allow_rust.scope = "crates/allow-rust/src/lib.rs".to_string();
    let rows = vec![allow_core, broad, allow_rust];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: Some(r"crates\allow-core"),
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("allow-core"));
    assert!(text.contains("allow-broad"));
    assert!(!text.contains("allow-rust"));
}
