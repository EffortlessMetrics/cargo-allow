#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ExplainContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
}
