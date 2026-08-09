use allow_core::MatchStatus;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WorkItemLedger {
    pub(super) ledger_id: Option<String>,
    pub(super) ledger_path: Option<String>,
    pub(super) lane: Option<String>,
    pub(super) mode: Option<String>,
    pub(super) role: Option<String>,
}

impl WorkItemLedger {
    pub(super) fn from_finding(finding: Option<&allow_core::Finding>) -> Self {
        finding
            .and_then(|finding| finding.ledger.as_ref())
            .map(|ledger| Self {
                ledger_id: Some(ledger.ledger_id.clone()),
                ledger_path: Some(ledger.ledger_path.clone()),
                lane: Some(ledger.lane.clone()),
                mode: Some(ledger.mode.clone()),
                role: Some(ledger.role.clone()),
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkItemEvidenceReference {
    pub(super) raw: String,
    pub(super) prefix: Option<String>,
    pub(super) target: Option<String>,
    pub(super) status: String,
    pub(super) category: String,
    pub(super) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkItem {
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
    pub(super) selector_precision: Option<u32>,
    pub(super) risk: &'static str,
    pub(super) difficulty: &'static str,
    pub(super) status: MatchStatus,
    pub(super) allow_id: Option<String>,
    pub(super) candidate_ids: Vec<String>,
    pub(super) finding_index: Option<usize>,
    pub(super) path: Option<String>,
    pub(super) line: Option<u32>,
    pub(super) column: Option<u32>,
    pub(super) evidence_reference: Option<WorkItemEvidenceReference>,
    pub(super) source_package: Option<String>,
    pub(super) message: String,
    pub(super) suggested_actions: Vec<String>,
    pub(super) proof_commands: Vec<String>,
    pub(super) ledger: WorkItemLedger,
}

#[derive(Debug, Clone, Copy, Default)]
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
    pub(super) broken_evidence: bool,
    pub(super) weak_evidence: bool,
}

impl WorklistFilters<'_> {
    /// Whether any filter narrowed the listing.
    ///
    /// A filtered queue only ever describes the slice it listed, so an empty
    /// filtered queue must not be reported as a clean repository.
    pub(super) fn any_active(&self) -> bool {
        self.kind.is_some()
            || self.family.is_some()
            || self.item_kind.is_some()
            || self.status.is_some()
            || self.allow_id.is_some()
            || self.path.is_some()
            || self.source_package.is_some()
            || self.owner.is_some()
            || self.classification.is_some()
            || self.risk.is_some()
            || self.difficulty.is_some()
            || self.baseline_debt
            || self.broad_scope
            || self.missing_evidence
            || self.broken_evidence
            || self.weak_evidence
    }
}
