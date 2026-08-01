use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// One TSV column in the `list` human-format output. Order of the variants
/// is the canonical column order emitted by `render_list_human` when no
/// `--columns` selection is made (#2595).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Status,
    Matches,
    Kind,
    Family,
    Owner,
    Classification,
    Scope,
    SourcePackage,
    EvidenceCount,
    BrokenEvidenceReferences,
    WeakEvidenceReferences,
    SelectorPrecision,
    BroadScope,
    ReviewAfter,
    Expires,
    Reason,
}

impl ListColumn {
    /// All columns in canonical order. Used when `--columns all` is supplied.
    pub const ALL: &'static [ListColumn] = &[
        ListColumn::Id,
        ListColumn::Status,
        ListColumn::Matches,
        ListColumn::Kind,
        ListColumn::Family,
        ListColumn::Owner,
        ListColumn::Classification,
        ListColumn::Scope,
        ListColumn::SourcePackage,
        ListColumn::EvidenceCount,
        ListColumn::BrokenEvidenceReferences,
        ListColumn::WeakEvidenceReferences,
        ListColumn::SelectorPrecision,
        ListColumn::BroadScope,
        ListColumn::ReviewAfter,
        ListColumn::Expires,
        ListColumn::Reason,
    ];

    /// Concise default projection: the most commonly needed columns for
    /// everyday policy review (#2787). Use `--columns all` for the full set.
    pub const DEFAULT: &'static [ListColumn] = &[
        ListColumn::Id,
        ListColumn::Status,
        ListColumn::Kind,
        ListColumn::Scope,
        ListColumn::Owner,
        ListColumn::Reason,
    ];

    /// The TSV header name for this column, matching the pre-#2595 header row.
    pub fn header(self) -> &'static str {
        match self {
            ListColumn::Id => "id",
            ListColumn::Status => "status",
            ListColumn::Matches => "matches",
            ListColumn::Kind => "kind",
            ListColumn::Family => "family",
            ListColumn::Owner => "owner",
            ListColumn::Classification => "classification",
            ListColumn::Scope => "scope",
            ListColumn::SourcePackage => "source_package",
            ListColumn::EvidenceCount => "evidence_count",
            ListColumn::BrokenEvidenceReferences => "broken_evidence_references",
            ListColumn::WeakEvidenceReferences => "weak_evidence_references",
            ListColumn::SelectorPrecision => "selector_precision",
            ListColumn::BroadScope => "broad_scope",
            ListColumn::ReviewAfter => "review_after",
            ListColumn::Expires => "expires",
            ListColumn::Reason => "reason",
        }
    }

    /// The cell value for this column from `row`. Mirrors the per-row
    /// formatting that was previously inline at `list.rs:47-66`: empty
    /// `owner`/`classification` collapse to `-`, and `Option<&str>` fields
    /// use `-` when absent.
    pub fn value<'a>(self, row: &'a ListRow<'a>) -> std::borrow::Cow<'a, str> {
        use std::borrow::Cow;
        match self {
            ListColumn::Id => sanitized(row.id),
            ListColumn::Status => Cow::Borrowed(row.status),
            ListColumn::Matches => Cow::Owned(row.matches.to_string()),
            ListColumn::Kind => Cow::Borrowed(row.kind),
            ListColumn::Family => sanitized(row.family.unwrap_or("-")),
            ListColumn::Owner => sanitized(empty_as_dash(row.owner)),
            ListColumn::Classification => sanitized(empty_as_dash(row.classification)),
            ListColumn::Scope => sanitized(row.scope),
            ListColumn::SourcePackage => sanitized(row.source_package.unwrap_or("-")),
            ListColumn::EvidenceCount => Cow::Owned(row.evidence_count.to_string()),
            ListColumn::BrokenEvidenceReferences => {
                Cow::Owned(row.broken_evidence_references.to_string())
            }
            ListColumn::WeakEvidenceReferences => {
                Cow::Owned(row.weak_evidence_references.to_string())
            }
            ListColumn::SelectorPrecision => Cow::Owned(row.selector_precision.to_string()),
            ListColumn::BroadScope => Cow::Owned(row.broad_scope.to_string()),
            ListColumn::ReviewAfter => sanitized(row.review_after.unwrap_or("-")),
            ListColumn::Expires => sanitized(row.expires.unwrap_or("-")),
            ListColumn::Reason => sanitized(row.reason),
        }
    }

    /// The bounded cell projection used by the CLI's concise human view.
    /// Explicit `--columns` and `--wide` retain the complete cell values.
    pub fn concise_value(self, row: &ListRow<'_>) -> String {
        self.concise_value_with_width(row, self.concise_width())
    }

    /// Render a concise cell within a caller-provided display-width budget.
    ///
    /// The list card renderer uses this when an operator explicitly supplies
    /// a terminal width. The normal concise limits remain the ceiling so a
    /// wider terminal does not make repository text unbounded.
    pub fn concise_value_with_width(self, row: &ListRow<'_>, max_width: usize) -> String {
        let value = self.value(row);
        truncate_with_ellipsis(value.as_ref(), max_width.min(self.concise_width()))
    }

    fn concise_width(self) -> usize {
        match self {
            ListColumn::Id => 36,
            ListColumn::Status => 20,
            ListColumn::Matches
            | ListColumn::EvidenceCount
            | ListColumn::BrokenEvidenceReferences
            | ListColumn::WeakEvidenceReferences
            | ListColumn::SelectorPrecision
            | ListColumn::BroadScope => 16,
            ListColumn::Kind | ListColumn::Family => 20,
            ListColumn::Owner | ListColumn::Classification | ListColumn::SourcePackage => 24,
            ListColumn::Scope => 48,
            ListColumn::ReviewAfter | ListColumn::Expires => 20,
            ListColumn::Reason => 72,
        }
    }

    /// Parse a comma-separated column selection (e.g. `"id,status,reason"`)
    /// into ordered variants. Trims whitespace around each name and matches
    /// case-insensitively (`ID` and `id` are equivalent). Returns an error
    /// listing the valid names on unknown input, empty input, or duplicate
    /// selections (#2595).
    pub fn parse_csv(input: &str) -> Result<Vec<ListColumn>, String> {
        // Special case: --columns all restores the full 17-column projection (#2787).
        if input.trim().eq_ignore_ascii_case("all") {
            return Ok(ListColumn::ALL.to_vec());
        }
        let mut seen = Vec::new();
        for raw in input.split(',') {
            let name = raw.trim();
            if name.is_empty() {
                return Err(format!(
                    "empty column name in --columns; valid columns: {}",
                    ListColumn::valid_names_joined()
                ));
            }
            let column = ListColumn::from_header(name).ok_or_else(|| {
                format!(
                    "unknown --columns name `{name}`; valid columns: {}",
                    ListColumn::valid_names_joined()
                )
            })?;
            if seen.contains(&column) {
                return Err(format!(
                    "duplicate --columns name `{name}`; valid columns: {}",
                    ListColumn::valid_names_joined()
                ));
            }
            seen.push(column);
        }
        if seen.is_empty() {
            return Err(format!(
                "no columns selected; valid columns: {}",
                ListColumn::valid_names_joined()
            ));
        }
        Ok(seen)
    }

    fn from_header(name: &str) -> Option<ListColumn> {
        // Case-insensitive match so `ID`, `Id`, `id` all resolve. The canonical
        // header names are lowercase; the error message lists those lowercase
        // names so the operator knows the expected spelling.
        ListColumn::ALL
            .iter()
            .copied()
            .find(|column| column.header().eq_ignore_ascii_case(name))
    }

    fn valid_names_joined() -> String {
        ListColumn::ALL
            .iter()
            .map(|column| column.header())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(crate) fn truncate_with_ellipsis(value: &str, max_width: usize) -> String {
    if value.width() <= max_width {
        return value.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let prefix_width = max_width.saturating_sub("…".width());
    let mut prefix = String::new();
    let mut width = 0;
    for grapheme in value.graphemes(true) {
        let grapheme_width = grapheme.width();
        if width + grapheme_width > prefix_width {
            break;
        }
        prefix.push_str(grapheme);
        width += grapheme_width;
    }
    format!("{prefix}…")
}

fn sanitized<'a>(value: &'a str) -> std::borrow::Cow<'a, str> {
    std::borrow::Cow::Owned(crate::style::sanitize_terminal_text(value))
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListFilters<'a> {
    pub kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub allow_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub expired: bool,
    pub review_due: bool,
    pub stale: bool,
    pub location_drift: bool,
    pub baseline_debt: bool,
    pub broad_scope: bool,
    pub missing_evidence: bool,
    pub broken_evidence: bool,
    pub weak_evidence: bool,
}

impl ListFilters<'_> {
    pub fn has_active_filter(self) -> bool {
        self.kind.is_some()
            || self.family.is_some()
            || self.owner.is_some()
            || self.classification.is_some()
            || self.path.is_some()
            || self.source_package.is_some()
            || self.allow_id.is_some()
            || self.status.is_some()
            || self.expired
            || self.review_due
            || self.stale
            || self.location_drift
            || self.baseline_debt
            || self.broad_scope
            || self.missing_evidence
            || self.broken_evidence
            || self.weak_evidence
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ListRow<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub matches: usize,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub owner: &'a str,
    pub classification: &'a str,
    pub scope: &'a str,
    pub source_package: Option<&'a str>,
    pub evidence_count: usize,
    pub broken_evidence_references: usize,
    pub weak_evidence_references: usize,
    pub selector_precision: u32,
    pub broad_scope: bool,
    pub review_after: Option<&'a str>,
    pub expires: Option<&'a str>,
    pub reason: &'a str,
}
