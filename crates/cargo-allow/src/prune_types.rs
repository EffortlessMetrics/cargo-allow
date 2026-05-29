use allow_core::FindingKind;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PruneContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PruneCandidate {
    pub(super) id: String,
    pub(super) kind: FindingKind,
    pub(super) family: Option<String>,
    pub(super) owner: String,
    pub(super) classification: String,
    pub(super) scope: String,
    pub(super) reason: String,
}
