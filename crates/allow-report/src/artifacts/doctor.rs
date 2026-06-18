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
pub struct DoctorReport<'a> {
    pub source_tree_root: &'a str,
    pub root_discovery: &'a str,
    pub config_path: Option<&'a str>,
    pub config_schema_version: Option<&'a str>,
    pub config_policy: Option<&'a str>,
    pub config_owner: Option<&'a str>,
    pub config_status: Option<&'a str>,
    pub config_valid: Option<bool>,
    pub config_diagnostic: Option<&'a str>,
    pub broken_evidence_links: Option<usize>,
    pub weak_evidence_references: Option<usize>,
    pub inventory_source: &'a str,
    pub files_scanned: usize,
    pub federation_config_path: Option<&'a str>,
    pub federation_config_found: bool,
    pub federation_config_valid: Option<bool>,
    pub configured_ledgers: Option<&'a [ConfiguredLedgerSummary<'a>]>,
    pub federation_diagnostics: Option<&'a [FederationDiagnosticSummary<'a>]>,
    pub federation_divergences: Option<&'a [FederationDiagnosticSummary<'a>]>,
}
