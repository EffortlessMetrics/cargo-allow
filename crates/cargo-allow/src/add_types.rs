#[derive(Debug, Clone, Default)]
pub(crate) struct AddContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    /// Source-tree root, for mutation-receipt provenance (GOAL-0004 PR 5A).
    pub(super) repo_root: Option<String>,
    /// Resolved policy config path, for mutation-receipt provenance.
    pub(super) config_source: Option<String>,
}
