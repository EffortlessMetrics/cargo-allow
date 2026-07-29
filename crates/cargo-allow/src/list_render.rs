use super::{ListContext, ListFilters, ListRow, list_filter::list_row_matches};

#[cfg(test)]
pub(super) fn render_list_rows(rows: &[ListRow], filters: &ListFilters<'_>) -> String {
    allow_report::render_list_human(
        &report_list_rows(rows, filters),
        allow_report::InventoryContext::unknown_source_syntax(),
    )
}

#[cfg(test)]
pub(super) fn render_list_rows_with_context(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    allow_report::render_list_human(&report_list_rows(rows, filters), context.inventory)
}

/// Render the list human-format TSV with a column subset (#2595).
/// `columns` is the resolved projection for an explicit human view.
pub(super) fn render_list_rows_with_columns(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
    columns: &[allow_report::ListColumn],
) -> String {
    allow_report::render_list_human_columns(
        &report_list_rows(rows, filters),
        context.inventory,
        columns,
    )
}

pub(super) fn render_list_rows_concise(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
    columns: &[allow_report::ListColumn],
) -> String {
    let report_rows = report_list_rows(rows, filters);
    allow_report::render_list_human_concise(
        &report_rows,
        context.inventory,
        report_list_filters(filters, context),
        columns,
    )
}

pub(super) fn render_list_rows_json(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    let report_rows = report_list_rows(rows, filters);
    allow_report::render_list_json(
        &report_rows,
        report_list_filters(filters, context),
        context.inventory,
    )
}

fn report_list_rows<'a>(
    rows: &'a [ListRow],
    filters: &ListFilters<'_>,
) -> Vec<allow_report::ListRow<'a>> {
    rows.iter()
        .filter(|row| list_row_matches(row, filters))
        .map(|row| allow_report::ListRow {
            id: &row.id,
            status: row.status.as_str(),
            matches: row.matches,
            kind: row.kind.as_str(),
            family: row.family.as_deref(),
            owner: &row.owner,
            classification: &row.classification,
            scope: &row.scope,
            source_package: row.source_package.as_deref(),
            evidence_count: row.evidence_count,
            broken_evidence_references: row.broken_evidence_references,
            weak_evidence_references: row.weak_evidence_references,
            selector_precision: row.selector_precision,
            broad_scope: row.broad_scope,
            review_after: dash_as_none(&row.review_after),
            expires: dash_as_none(&row.expires),
            reason: &row.reason,
        })
        .collect()
}

fn dash_as_none(value: &str) -> Option<&str> {
    if value == "-" { None } else { Some(value) }
}

fn report_list_filters<'a>(
    filters: &'a ListFilters<'a>,
    context: ListContext<'a>,
) -> allow_report::ListFilters<'a> {
    allow_report::ListFilters {
        kind: context.kind_arg,
        family: filters.family,
        owner: filters.owner,
        classification: filters.classification,
        path: filters.path,
        source_package: filters.source_package,
        allow_id: filters.allow_id,
        status: filters.status,
        expired: filters.expired,
        review_due: filters.review_due,
        stale: filters.stale,
        location_drift: filters.location_drift,
        baseline_debt: filters.baseline_debt,
        broad_scope: filters.broad_scope,
        missing_evidence: filters.missing_evidence,
        broken_evidence: filters.broken_evidence,
        weak_evidence: filters.weak_evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, MatchStatus};

    fn filters() -> ListFilters<'static> {
        ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            allow_id: Some("allow-keep"),
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

    fn row(id: &str) -> ListRow {
        ListRow {
            id: id.to_string(),
            status: MatchStatus::Matched,
            matches: 2,
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            owner: "parser".to_string(),
            classification: "approved".to_string(),
            scope: "src/lib.rs".to_string(),
            source_package: Some("allow-core".to_string()),
            evidence_count: 3,
            broken_evidence_references: 1,
            weak_evidence_references: 2,
            selector_precision: 7,
            broad_scope: true,
            review_after: "-".to_string(),
            expires: "2026-12-01".to_string(),
            reason: "reason".to_string(),
        }
    }

    #[test]
    fn report_list_rows_filters_and_projects_all_report_fields() {
        let rows = [row("allow-keep"), row("allow-skip")];

        let report_rows = report_list_rows(&rows, &filters());

        assert_eq!(report_rows.len(), 1);
        let row = report_rows
            .first()
            .copied()
            .unwrap_or_else(|| std::panic::panic_any("expected one projected list row"));
        assert_eq!(row.id, "allow-keep");
        assert_eq!(row.status, "matched");
        assert_eq!(row.matches, 2);
        assert_eq!(row.kind, "panic");
        assert_eq!(row.family, Some("unwrap"));
        assert_eq!(row.owner, "parser");
        assert_eq!(row.classification, "approved");
        assert_eq!(row.scope, "src/lib.rs");
        assert_eq!(row.source_package, Some("allow-core"));
        assert_eq!(row.evidence_count, 3);
        assert_eq!(row.broken_evidence_references, 1);
        assert_eq!(row.weak_evidence_references, 2);
        assert_eq!(row.selector_precision, 7);
        assert!(row.broad_scope);
        assert_eq!(row.review_after, None);
        assert_eq!(row.expires, Some("2026-12-01"));
        assert_eq!(row.reason, "reason");
    }
}
