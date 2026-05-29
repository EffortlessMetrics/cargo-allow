use allow_core::{FindingKind, MatchStatus};

use crate::KindFilter;

#[derive(Debug, Clone)]
pub(super) struct ListRow {
    pub(super) id: String,
    pub(super) status: MatchStatus,
    pub(super) matches: usize,
    pub(super) kind: FindingKind,
    pub(super) family: Option<String>,
    pub(super) owner: String,
    pub(super) classification: String,
    pub(super) scope: String,
    pub(super) source_package: Option<String>,
    pub(super) evidence_count: usize,
    pub(super) review_after: String,
    pub(super) expires: String,
    pub(super) reason: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ListFilters<'a> {
    pub(super) kind: Option<KindFilter>,
    pub(super) family: Option<&'a str>,
    pub(super) owner: Option<&'a str>,
    pub(super) classification: Option<&'a str>,
    pub(super) path: Option<&'a str>,
    pub(super) source_package: Option<&'a str>,
    pub(super) status: Option<&'a str>,
    pub(super) expired: bool,
    pub(super) review_due: bool,
    pub(super) stale: bool,
    pub(super) baseline_debt: bool,
    pub(super) broad_scope: bool,
    pub(super) missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ListContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) kind_arg: Option<&'a str>,
}

impl<'a> Default for ListContext<'a> {
    fn default() -> Self {
        Self {
            inventory: allow_report::InventoryContext::unknown_source_syntax(),
            kind_arg: None,
        }
    }
}
