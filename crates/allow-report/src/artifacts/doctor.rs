#[derive(Debug, Clone, Copy)]
pub struct DoctorReport<'a> {
    pub source_tree_root: &'a str,
    pub root_discovery: &'a str,
    pub config_path: Option<&'a str>,
    pub inventory_source: &'a str,
    pub files_scanned: usize,
}
