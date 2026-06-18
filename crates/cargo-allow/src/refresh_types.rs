use allow_report::InventoryContext;

#[derive(Debug, Clone, Copy)]
pub(super) struct RefreshContext<'a> {
    pub inventory: InventoryContext<'a>,
}
