#[derive(Debug, Clone, Copy)]
pub(crate) struct AddContext<'a> {
    pub(super) inventory_source: &'a str,
    pub(super) source_tree_root: Option<&'a str>,
    pub(super) inventory_files: Option<usize>,
}

impl<'a> Default for AddContext<'a> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}
