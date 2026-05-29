use allow_core::FindingKind;

#[derive(Debug, Clone, Copy)]
pub(super) struct PruneContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}

impl<'a> Default for PruneContext<'a> {
    fn default() -> Self {
        Self {
            inventory: allow_report::InventoryContext::unknown_source_syntax(),
        }
    }
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
