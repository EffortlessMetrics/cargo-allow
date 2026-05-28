#[derive(Debug, Clone, Copy)]
pub(crate) struct DoctorFacts<'a> {
    pub(super) source_tree_root: &'a str,
    pub(super) root_discovery: &'a str,
    pub(super) config_path: Option<&'a str>,
    pub(super) inventory_source: &'a str,
    pub(super) files_scanned: usize,
}
