#[derive(Debug, Clone, Copy)]
pub(crate) struct AddContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}

impl<'a> Default for AddContext<'a> {
    fn default() -> Self {
        Self {
            inventory: allow_report::InventoryContext::unknown_source_syntax(),
        }
    }
}
