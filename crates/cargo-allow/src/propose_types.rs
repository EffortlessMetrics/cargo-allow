#[derive(Debug, Clone, Copy)]
pub(crate) struct ProposeContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) kind_filter: Option<&'a str>,
}

impl<'a> Default for ProposeContext<'a> {
    fn default() -> Self {
        Self {
            inventory: allow_report::InventoryContext::unknown_source_syntax(),
            kind_filter: None,
        }
    }
}
