use super::test_support::list_row;
use super::*;

#[test]
fn render_list_rows_filters_broad_scope() {
    let mut exact = list_row("allow-exact", FindingKind::Panic, "parser", "baseline_debt");
    exact.scope = "crates/allow-core/src/lib.rs".to_string();
    let mut broad = list_row(
        "allow-broad",
        FindingKind::NonRustFile,
        "tools",
        "baseline_debt",
    );
    broad.scope = "crates/allow-core/**".to_string();
    broad.broad_scope = true;
    let rows = vec![exact, broad];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: true,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-exact"));
    assert!(text.contains("allow-broad"));
}

#[test]
fn render_list_rows_reports_and_filters_missing_evidence() {
    let mut missing = list_row(
        "allow-missing",
        FindingKind::Panic,
        "parser",
        "baseline_debt",
    );
    missing.evidence_count = 0;
    let mut evidenced = list_row(
        "allow-evidenced",
        FindingKind::Unsafe,
        "runtime",
        "reviewed_exception",
    );
    evidenced.evidence_count = 2;
    let rows = vec![missing, evidenced];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: true,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("evidence_count"));
    assert!(text.contains("allow-missing"));
    assert!(!text.contains("allow-evidenced"));
}
