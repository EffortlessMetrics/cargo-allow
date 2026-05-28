use allow_core::MatchStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkItem {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) exception_kind: Option<String>,
    pub(super) family: Option<String>,
    pub(super) owner: Option<String>,
    pub(super) classification: Option<String>,
    pub(super) reason: Option<String>,
    pub(super) created: Option<String>,
    pub(super) review_after: Option<String>,
    pub(super) expires: Option<String>,
    pub(super) evidence_count: Option<usize>,
    pub(super) risk: &'static str,
    pub(super) difficulty: &'static str,
    pub(super) status: MatchStatus,
    pub(super) allow_id: Option<String>,
    pub(super) finding_index: Option<usize>,
    pub(super) path: Option<String>,
    pub(super) source_package: Option<String>,
    pub(super) message: String,
    pub(super) suggested_actions: Vec<String>,
    pub(super) proof_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WorklistContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) filters: WorklistFilters<'a>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct WorklistFilters<'a> {
    pub(super) kind: Option<&'a str>,
    pub(super) family: Option<&'a str>,
    pub(super) item_kind: Option<&'a str>,
    pub(super) status: Option<&'a str>,
    pub(super) allow_id: Option<&'a str>,
    pub(super) path: Option<&'a str>,
    pub(super) source_package: Option<&'a str>,
    pub(super) owner: Option<&'a str>,
    pub(super) classification: Option<&'a str>,
    pub(super) baseline_debt: bool,
    pub(super) broad_scope: bool,
    pub(super) risk: Option<&'a str>,
    pub(super) difficulty: Option<&'a str>,
    pub(super) missing_evidence: bool,
}

impl<'a> Default for WorklistContext<'a> {
    fn default() -> Self {
        Self {
            inventory: allow_report::InventoryContext::source_syntax("unknown", None, None),
            filters: WorklistFilters::default(),
        }
    }
}
