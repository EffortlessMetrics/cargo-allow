use allow_core::AllowConfig;

#[derive(Debug, Clone)]
pub(super) struct MigrationLoad {
    pub(super) cfg: AllowConfig,
    pub(super) context: MigrateContext,
}

#[derive(Debug, Clone)]
pub(super) struct MigrateContext {
    pub(super) inventory_source: String,
    pub(super) source_tree_root: Option<String>,
    pub(super) inventory_files: Option<usize>,
    pub(super) input_kind: String,
    pub(super) input_path: String,
}
