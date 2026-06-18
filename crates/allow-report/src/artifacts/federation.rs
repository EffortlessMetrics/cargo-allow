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
pub struct FederationReportContext<'a> {
    pub federation_version: Option<&'a str>,
    pub precedence_applied: Option<&'a str>,
    pub ledger_contributors: Option<&'a [LedgerContributorSummary<'a>]>,
}
