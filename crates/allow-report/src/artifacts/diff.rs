#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffPostureSummary {
    pub current_failures: usize,
    pub new_findings: usize,
    pub removed_findings: usize,
    pub policy_failures: usize,
    pub policy_review_items: usize,
    pub policy_improvements: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffFindingChange<'a> {
    pub change: &'a str,
    pub key: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub path: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffPolicyChange<'a> {
    pub severity: &'a str,
    pub allow_id: &'a str,
    pub kind: &'a str,
    pub message: &'a str,
    pub selector_precision: Option<DiffSelectorPrecisionChange<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffSelectorPrecisionChange<'a> {
    pub before: u32,
    pub after: u32,
    pub removed_fields: &'a [&'a str],
    pub added_fields: &'a [&'a str],
}

#[derive(Debug, Clone, Copy)]
pub struct DiffReport<'a> {
    pub net_posture: &'a str,
    pub reviewer_action: &'a str,
    pub summary: DiffPostureSummary,
    pub finding_changes: &'a [DiffFindingChange<'a>],
    pub policy_changes: &'a [DiffPolicyChange<'a>],
}
