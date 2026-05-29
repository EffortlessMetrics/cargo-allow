#[derive(Debug, Clone, Copy, Default)]
pub struct ListFilters<'a> {
    pub kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub status: Option<&'a str>,
    pub expired: bool,
    pub review_due: bool,
    pub stale: bool,
    pub baseline_debt: bool,
    pub broad_scope: bool,
    pub missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ListRow<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub matches: usize,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub owner: &'a str,
    pub classification: &'a str,
    pub scope: &'a str,
    pub source_package: Option<&'a str>,
    pub evidence_count: usize,
    pub review_after: Option<&'a str>,
    pub expires: Option<&'a str>,
    pub reason: &'a str,
}
