use super::{ListContext, ListFilters, ListRow};
use allow_core::{MatchStatus, source_tree_path_matches_filter, source_tree_scope_has_wildcard};

pub(super) fn render_list_rows(rows: &[ListRow], filters: &ListFilters<'_>) -> String {
    allow_report::render_list_human(&report_list_rows(rows, filters))
}

pub(super) fn render_list_rows_json(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    let report_rows = report_list_rows(rows, filters);
    allow_report::render_list_json(
        &report_rows,
        allow_report::ListFilters {
            kind: context.kind_arg,
            family: filters.family,
            owner: filters.owner,
            classification: filters.classification,
            path: filters.path,
            source_package: filters.source_package,
            status: filters.status,
            expired: filters.expired,
            review_due: filters.review_due,
            stale: filters.stale,
            baseline_debt: filters.baseline_debt,
            broad_scope: filters.broad_scope,
            missing_evidence: filters.missing_evidence,
        },
        allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
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
            review_after: dash_as_none(&row.review_after),
            expires: dash_as_none(&row.expires),
            reason: &row.reason,
        })
        .collect()
}

fn dash_as_none(value: &str) -> Option<&str> {
    if value == "-" { None } else { Some(value) }
}

fn list_row_matches(row: &ListRow, filters: &ListFilters<'_>) -> bool {
    if let Some(kind) = filters.kind {
        if row.kind != kind.kind || !kind.family.matches(row.family.as_deref()) {
            return false;
        }
    }
    if let Some(family) = filters.family {
        if row.family.as_deref() != Some(family) {
            return false;
        }
    }
    if let Some(owner) = filters.owner {
        if row.owner != owner {
            return false;
        }
    }
    if let Some(classification) = filters.classification {
        if row.classification != classification {
            return false;
        }
    }
    if let Some(path) = filters.path {
        if !source_tree_path_matches_filter(&row.scope, path) {
            return false;
        }
    }
    if let Some(source_package) = filters.source_package {
        if row.source_package.as_deref() != Some(source_package) {
            return false;
        }
    }
    if let Some(status) = filters.status {
        if row.status.as_str() != status {
            return false;
        }
    }
    if filters.expired && row.status != MatchStatus::Expired {
        return false;
    }
    if filters.review_due && row.status != MatchStatus::ReviewDue {
        return false;
    }
    if filters.stale && row.status != MatchStatus::Stale {
        return false;
    }
    if filters.baseline_debt && row.classification != "baseline_debt" {
        return false;
    }
    if filters.broad_scope && !source_tree_scope_has_wildcard(&row.scope) {
        return false;
    }
    if filters.missing_evidence && row.evidence_count != 0 {
        return false;
    }
    true
}
