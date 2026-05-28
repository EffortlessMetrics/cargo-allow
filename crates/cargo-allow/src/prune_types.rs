use allow_core::FindingKind;

#[derive(Debug, Clone, Copy)]
pub(super) struct PruneContext<'a> {
    pub(super) inventory_source: &'a str,
    pub(super) source_tree_root: Option<&'a str>,
    pub(super) inventory_files: Option<usize>,
}

impl<'a> Default for PruneContext<'a> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PruneCandidate {
    pub(super) id: String,
    pub(super) kind: FindingKind,
    pub(super) family: Option<String>,
    pub(super) owner: String,
    pub(super) classification: String,
    pub(super) scope: String,
    pub(super) reason: String,
}
