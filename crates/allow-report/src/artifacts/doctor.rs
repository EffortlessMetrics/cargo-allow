#[derive(Debug, Clone, Copy)]
pub struct ConfiguredLedgerSummary<'a> {
    pub id: &'a str,
    pub path: &'a str,
    pub dialect: &'a str,
    pub role: &'a str,
    pub mode: &'a str,
    pub priority: u32,
    pub lanes: &'a [String],
    pub mirrors: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct FederationDiagnosticSummary<'a> {
    pub kind: &'a str,
    pub message: &'a str,
    pub ledger_ids: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct FileFamilyRuleSummary<'a> {
    pub id: &'a str,
    pub family: &'a str,
    pub glob: &'a str,
    pub matched_files: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct FileFamilyConflictSummary<'a> {
    pub path: &'a str,
    pub rule_ids: &'a [String],
    pub families: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct ConfigProvenanceSummary<'a> {
    pub source: &'a str,
    pub precedence: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct DoctorReport<'a> {
    pub source_tree_root: &'a str,
    pub root_discovery: &'a str,
    pub config_path: Option<&'a str>,
    pub config_schema_version: Option<&'a str>,
    pub config_policy: Option<&'a str>,
    pub config_owner: Option<&'a str>,
    pub config_status: Option<&'a str>,
    pub config_provenance: Option<ConfigProvenanceSummary<'a>>,
    pub config_valid: Option<bool>,
    pub config_diagnostic: Option<&'a str>,
    pub broken_evidence_links: Option<usize>,
    pub weak_evidence_references: Option<usize>,
    pub inventory_source: &'a str,
    pub inventory_completeness: &'a str,
    pub files_scanned: usize,
    /// Git inventory succeeded but reported no tracked files (#1849).
    pub empty_git_tracked: bool,
    /// Git-tracked paths absent from the worktree (#2048). Surfaced so a scan
    /// never looks complete while a tracked path disappeared from coverage.
    pub deleted_tracked_files: usize,
    /// Git error message when the inventory fell back from git-tracked to
    /// filesystem scanning (#1845). Empty string when git succeeded.
    pub git_inventory_error: Option<&'a str>,
    /// Count of paths skipped during filesystem traversal due to I/O errors
    /// (#1844).
    pub skipped_paths: usize,
    /// Count of detected submodule gitlinks (checked-out directories that are
    /// git-tracked). Their contents are not scanned (#1846).
    pub submodule_paths: usize,
    /// Completeness of the Rust source scan, independent of inventory
    /// traversal completeness. `unknown` means no Rust files were selected.
    pub rust_scanner_completeness: &'a str,
    pub rust_files_considered: usize,
    pub rust_files_scanned: usize,
    pub rust_files_skipped: usize,
    pub rust_files_with_parse_errors: usize,
    /// Bounded aggregate for read, encoding, and size failures.
    pub rust_files_skipped_by_read_or_unsupported: usize,
    pub federation_config_path: Option<&'a str>,
    pub federation_config_found: bool,
    pub federation_config_valid: Option<bool>,
    pub configured_ledgers: Option<&'a [ConfiguredLedgerSummary<'a>]>,
    pub federation_diagnostics: Option<&'a [FederationDiagnosticSummary<'a>]>,
    pub federation_divergences: Option<&'a [FederationDiagnosticSummary<'a>]>,
    pub file_family_rules: &'a [FileFamilyRuleSummary<'a>],
    pub file_family_conflicts: &'a [FileFamilyConflictSummary<'a>],
}
