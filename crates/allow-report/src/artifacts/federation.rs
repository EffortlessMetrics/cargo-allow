#[derive(Debug, Clone, Copy)]
pub struct LedgerContributorSummary<'a> {
    pub id: &'a str,
    pub path: &'a str,
    pub role: &'a str,
    pub dialect: &'a str,
    pub mode: &'a str,
    pub priority: u32,
    pub lanes: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct FederationDivergenceRecordSummary<'a> {
    pub kind: &'a str,
    pub message: &'a str,
    pub canonical_ledger_id: &'a str,
    pub mirror_ledger_id: &'a str,
    pub canonical_path: &'a str,
    pub mirror_path: &'a str,
    pub sample_entry_ids: &'a [String],
    pub canonical_fingerprint: Option<&'a str>,
    pub mirror_fingerprint: Option<&'a str>,
    pub recommended_action: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct FederationDivergenceKindCount<'a> {
    pub kind: &'a str,
    pub count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct FederationDivergenceSummary<'a> {
    pub records: Option<&'a [FederationDivergenceRecordSummary<'a>]>,
    pub counts_by_kind: Option<&'a [FederationDivergenceKindCount<'a>]>,
}

#[derive(Debug, Clone, Copy)]
pub struct FederationReportContext<'a> {
    pub federation_version: Option<&'a str>,
    pub precedence_applied: Option<&'a str>,
    pub ledger_contributors: Option<&'a [LedgerContributorSummary<'a>]>,
    pub divergence_summary: Option<FederationDivergenceSummary<'a>>,
}
