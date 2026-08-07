use allow_core::AllowConfig;
use allow_report::MigrateBaselineDebtProjection;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(super) struct MigrationLoad {
    pub(super) cfg: AllowConfig,
    pub(super) context: MigrateContext,
    /// The resolved source-tree root, used for evidence reference validation
    /// (#1871). `None` when no root was resolved (e.g. legacy fallback).
    pub(super) root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(super) struct MigrateContext {
    pub(super) inventory_source: String,
    pub(super) source_tree_root: Option<String>,
    pub(super) inventory_files: Option<usize>,
    pub(super) input_kind: String,
    pub(super) input_path: String,
    pub(super) legacy_source_files: Vec<String>,
    pub(super) legacy_compat_kinds: Vec<&'static str>,
    /// Baseline-debt closeout projection computed from the legacy lane
    /// descriptors at load time. Threaded into `allow-report` so it can render
    /// closeout queues without depending on `allow-policy-legacy` (#2941).
    pub(super) baseline_debt_projection: MigrateBaselineDebtProjection,
}
