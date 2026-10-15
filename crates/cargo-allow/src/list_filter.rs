use allow_core::{MatchStatus, source_tree_path_matches_filter};

use super::{ListFilters, ListRow};

/// Fail closed on free-text filter values no ledger row carries (#4365).
///
/// Mirrors the worklist contract: `--family` and `--source-package` draw
/// from whatever the scanner and policy produced, so an unknown value is
/// almost certainly a typo. Silently rendering an empty table made that
/// indistinguishable from a genuinely empty slice.
pub(super) fn validate_list_free_text_filters(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
) -> allow_core::CargoAllowResult<()> {
    // An empty ledger keeps its empty-ledger summary semantics (the
    // list.no_entries vs list.no_filter_matches distinction); fail-closed
    // applies only when the ledger carries rows a typo could hide behind.
    if rows.is_empty() {
        return Ok(());
    }
    validate_filter_value(
        "--family",
        "family",
        filters.family,
        rows.iter().filter_map(|row| row.family.as_deref()),
    )?;
    validate_filter_value(
        "--source-package",
        "source package",
        filters.source_package,
        rows.iter().filter_map(|row| row.source_package.as_deref()),
    )?;
    validate_filter_value(
        "--owner",
        "owner",
        filters.owner,
        rows.iter().map(|row| row.owner.as_str()),
    )?;
    validate_filter_value(
        "--classification",
        "classification",
        filters.classification,
        rows.iter().map(|row| row.classification.as_str()),
    )
}

fn validate_filter_value<'a>(
    flag: &str,
    noun: &str,
    value: Option<&str>,
    observed: impl Iterator<Item = &'a str>,
) -> allow_core::CargoAllowResult<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let mut observed = observed.collect::<Vec<_>>();
    observed.sort_unstable();
    observed.dedup();
    if observed.contains(&value) {
        return Ok(());
    }
    let observed_text = if observed.is_empty() {
        "none (the ledger carries no values for this filter)".to_string()
    } else {
        observed.join(", ")
    };
    Err(allow_core::CargoAllowError::with_kind(
        allow_core::CargoAllowErrorKind::Usage,
        format!(
            "unknown {flag} value `{value}`: no allow entries carry {noun} `{value}`; observed {noun} values: {observed_text}. Run `cargo-allow list` without {flag} to see the ledger, or correct the {flag} value."
        ),
    ))
}

pub(super) fn list_row_matches(row: &ListRow, filters: &ListFilters<'_>) -> bool {
    if let Some(kind) = filters.kind
        && (row.kind != kind.kind || !kind.family.matches(row.family.as_deref()))
    {
        return false;
    }
    if let Some(family) = filters.family
        && row.family.as_deref() != Some(family)
    {
        return false;
    }
    if let Some(owner) = filters.owner
        && row.owner != owner
    {
        return false;
    }
    if let Some(classification) = filters.classification
        && row.classification != classification
    {
        return false;
    }
    if let Some(path) = filters.path
        && !source_tree_path_matches_filter(&row.scope, path)
    {
        return false;
    }
    if let Some(source_package) = filters.source_package
        && row.source_package.as_deref() != Some(source_package)
    {
        return false;
    }
    if let Some(allow_id) = filters.allow_id
        && row.id != allow_id
    {
        return false;
    }
    if let Some(status) = filters.status
        && row.status.as_str() != status
    {
        return false;
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
    if filters.location_drift && row.status != MatchStatus::LocationDrift {
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
    if filters.broken_evidence && row.broken_evidence_references == 0 {
        return false;
    }
    if filters.weak_evidence && row.weak_evidence_references == 0 {
        return false;
    }
    true
}
