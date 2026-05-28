#[derive(Debug, Clone, Copy)]
pub(crate) struct ProposeContext<'a> {
    pub(super) inventory_source: &'a str,
    pub(super) source_tree_root: Option<&'a str>,
    pub(super) inventory_files: Option<usize>,
    pub(super) kind_filter: Option<&'a str>,
}

impl<'a> Default for ProposeContext<'a> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            kind_filter: None,
        }
    }
}
