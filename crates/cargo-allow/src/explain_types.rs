#[derive(Debug, Clone, Copy)]
pub(crate) struct ExplainContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}

impl<'a> Default for ExplainContext<'a> {
    fn default() -> Self {
        Self {
            inventory: crate::reporting::unknown_source_syntax_inventory(),
        }
    }
}
