#[derive(Debug, Clone, Copy)]
pub(crate) struct ProposeContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) kind_filter: Option<&'a str>,
}

impl<'a> Default for ProposeContext<'a> {
    fn default() -> Self {
        Self {
            inventory: crate::reporting::unknown_source_syntax_inventory(),
            kind_filter: None,
        }
    }
}
