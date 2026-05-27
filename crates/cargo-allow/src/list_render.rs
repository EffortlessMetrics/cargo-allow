use super::{ListContext, ListFilters, ListRow};
use crate::{scope_has_wildcard, source_tree_path_matches_filter};
use allow_core::MatchStatus;

pub(super) fn render_list_rows(rows: &[ListRow], filters: &ListFilters<'_>) -> String {
    let mut out = String::new();
    out.push_str("id\tstatus\tmatches\tkind\tfamily\towner\tclassification\tscope\tsource_package\tevidence_count\treview_after\texpires\treason\n");
    let mut count = 0;
    for row in rows.iter().filter(|row| list_row_matches(row, filters)) {
        count += 1;
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.id,
            row.status.as_str(),
            row.matches,
            row.kind,
            row.family.as_deref().unwrap_or("-"),
            empty_as_dash(&row.owner),
            empty_as_dash(&row.classification),
            row.scope,
            row.source_package.as_deref().unwrap_or("-"),
            row.evidence_count,
            row.review_after,
            row.expires,
            row.reason
        ));
    }
    if count == 0 {
        out.push_str("(no allow entries matched filters)\n");
    }
    out
}

pub(super) fn render_list_rows_json(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    let filtered = rows
        .iter()
        .filter(|row| list_row_matches(row, filters))
        .collect::<Vec<_>>();
    let report_rows = filtered
        .iter()
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
        .collect::<Vec<_>>();
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
    if filters.broad_scope && !scope_has_wildcard(&row.scope) {
        return false;
    }
    if filters.missing_evidence && row.evidence_count != 0 {
        return false;
    }
    true
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}
