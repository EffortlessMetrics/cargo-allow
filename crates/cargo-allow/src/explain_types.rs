#[derive(Debug, Clone, Copy)]
pub(crate) struct ExplainContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}

impl<'a> Default for ExplainContext<'a> {
    fn default() -> Self {
        Self {
            inventory: allow_report::InventoryContext::source_syntax("unknown", None, None),
        }
    }
}
