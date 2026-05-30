use allow_core::{MatchStatus, source_tree_path_matches_filter};

use super::{ListFilters, ListRow};

pub(super) fn list_row_matches(row: &ListRow, filters: &ListFilters<'_>) -> bool {
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
    if let Some(allow_id) = filters.allow_id {
        if row.id != allow_id {
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
    if filters.broad_scope && !row.broad_scope {
        return false;
    }
    if filters.missing_evidence && row.evidence_count != 0 {
        return false;
    }
    true
}
