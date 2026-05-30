use super::test_support::list_row;
use super::*;

#[test]
fn render_list_rows_filters_owner_kind_classification_and_baseline_debt() {
    let rows = vec![
        list_row(
            "allow-runtime",
            FindingKind::Unsafe,
            "runtime",
            "baseline_debt",
        ),
        list_row(
            "allow-parser",
            FindingKind::Panic,
            "parser",
            "reviewed_exception",
        ),
    ];
    let filters = ListFilters {
        kind: Some(parse_kind_filter("unsafe").unwrap_or_else(|err| {
            std::panic::panic_any(format!("kind filter should parse: {err}"))
        })),
        family: None,
        owner: Some("runtime"),
        classification: Some("baseline_debt"),
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: true,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("allow-runtime"));
    assert!(!text.contains("allow-parser"));
    assert!(text.contains("baseline_debt"));
}

#[test]
fn render_list_rows_filters_classification_without_baseline_shortcut() {
    let rows = vec![
        list_row(
            "allow-runtime",
            FindingKind::Unsafe,
            "runtime",
            "baseline_debt",
        ),
        list_row(
            "allow-parser",
            FindingKind::Panic,
            "parser",
            "reviewed_exception",
        ),
    ];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: Some("reviewed_exception"),
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-runtime"));
    assert!(text.contains("allow-parser"));
    assert!(text.contains("reviewed_exception"));
}

#[test]
fn render_list_rows_filters_status() {
    let mut baseline = list_row(
        "allow-baseline",
        FindingKind::Panic,
        "parser",
        "baseline_debt",
    );
    baseline.status = MatchStatus::BaselineDebt;
    let mut stale = list_row(
        "allow-stale",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    );
    stale.status = MatchStatus::Stale;
    let rows = vec![baseline, stale];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: Some("stale"),
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-baseline"));
    assert!(text.contains("allow-stale"));
}

#[test]
fn render_list_rows_filters_allow_id() {
    let rows = vec![
        list_row(
            "allow-runtime",
            FindingKind::Unsafe,
            "runtime",
            "reviewed_exception",
        ),
        list_row(
            "allow-parser",
            FindingKind::Panic,
            "parser",
            "reviewed_exception",
        ),
    ];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: Some("allow-parser"),
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-runtime"));
    assert!(text.contains("allow-parser"));
}
