#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AddContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}
