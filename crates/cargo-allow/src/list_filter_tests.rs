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
        location_drift: false,
        baseline_debt: true,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
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
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
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
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-baseline"));
    assert!(text.contains("allow-stale"));
}

#[test]
fn render_list_rows_filters_location_drift_shortcut() {
    let mut matched = list_row(
        "allow-matched",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    );
    matched.status = MatchStatus::Matched;
    let mut drifted = list_row(
        "allow-drifted",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    );
    drifted.status = MatchStatus::LocationDrift;
    let rows = vec![matched, drifted];
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
        location_drift: true,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-matched"));
    assert!(text.contains("allow-drifted"));
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
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-runtime"));
    assert!(text.contains("allow-parser"));
}

fn filters_with<'a>(family: Option<&'a str>, source_package: Option<&'a str>) -> ListFilters<'a> {
    ListFilters {
        kind: None,
        family,
        owner: None,
        classification: None,
        path: None,
        source_package,
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
    }
}

#[test]
fn list_free_text_filters_fail_closed_on_unknown_values() {
    let mut first = list_row(
        "allow-one",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    );
    first.family = Some("unwrap".to_string());
    first.source_package = Some("allow-core".to_string());
    let mut second = list_row(
        "allow-two",
        FindingKind::Unsafe,
        "runtime",
        "reviewed_exception",
    );
    second.family = Some("unsafe_block".to_string());
    let rows = vec![first, second];

    // Unset and known values pass.
    assert!(list_filter::validate_list_free_text_filters(&rows, &filters_with(None, None)).is_ok());
    assert!(
        list_filter::validate_list_free_text_filters(
            &rows,
            &filters_with(Some("unwrap"), Some("allow-core")),
        )
        .is_ok()
    );

    // Unknown family fails closed and names the observed values.
    let error = list_filter::validate_list_free_text_filters(
        &rows,
        &filters_with(Some("bogus_family"), None),
    )
    .unwrap_err();
    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(
        error
            .to_string()
            .contains("unknown --family value `bogus_family`"),
        "{error}"
    );
    assert!(error.to_string().contains("unwrap"), "{error}");
    assert!(error.to_string().contains("unsafe_block"), "{error}");

    // Unknown source package fails closed the same way.
    let error =
        list_filter::validate_list_free_text_filters(&rows, &filters_with(None, Some("nope")))
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unknown --source-package value `nope`"),
        "{error}"
    );
    assert!(error.to_string().contains("allow-core"), "{error}");

    // Unknown owner and classification fail closed the same way.
    let mut owner_filters = filters_with(None, None);
    owner_filters.owner = Some("nobody");
    let error = list_filter::validate_list_free_text_filters(&rows, &owner_filters).unwrap_err();
    assert!(
        error.to_string().contains("unknown --owner value `nobody`"),
        "{error}"
    );
    let mut class_filters = filters_with(None, None);
    class_filters.classification = Some("nope");
    let error = list_filter::validate_list_free_text_filters(&rows, &class_filters).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unknown --classification value `nope`"),
        "{error}"
    );
    assert!(error.to_string().contains("reviewed_exception"), "{error}");

    // An empty ledger keeps empty-ledger semantics instead of erroring:
    // there is no slice a typo could be confused with.
    assert!(
        list_filter::validate_list_free_text_filters(&[], &filters_with(Some("unwrap"), None))
            .is_ok()
    );
}
