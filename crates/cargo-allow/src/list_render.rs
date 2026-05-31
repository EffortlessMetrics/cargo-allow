use super::{ListContext, ListFilters, ListRow, list_filter::list_row_matches};

#[cfg(test)]
pub(super) fn render_list_rows(rows: &[ListRow], filters: &ListFilters<'_>) -> String {
    allow_report::render_list_human(
        &report_list_rows(rows, filters),
        allow_report::InventoryContext::unknown_source_syntax(),
    )
}

pub(super) fn render_list_rows_with_context(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    allow_report::render_list_human(&report_list_rows(rows, filters), context.inventory)
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
        baseline_debt: filters.baseline_debt,
        broad_scope: filters.broad_scope,
        missing_evidence: filters.missing_evidence,
    }
}
