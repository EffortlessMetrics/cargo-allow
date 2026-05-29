#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProposeContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) kind_filter: Option<&'a str>,
}
