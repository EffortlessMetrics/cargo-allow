use allow_core::StructuralIdentity;

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
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub source_package: Option<&'a str>,
    pub identity: Option<&'a StructuralIdentity>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffPolicyChange<'a> {
    pub severity: &'a str,
    pub allow_id: &'a str,
    pub kind: &'a str,
    pub message: &'a str,
    pub exception_identity: Option<DiffExceptionIdentityChange<'a>>,
    pub selector_identity: Option<DiffSelectorIdentityChange<'a>>,
    pub selector_precision: Option<DiffSelectorPrecisionChange<'a>>,
    pub scope: Option<DiffScopeChange<'a>>,
    pub occurrence_limit: Option<DiffOccurrenceLimitChange>,
    pub lifecycle: Option<DiffLifecycleChange<'a>>,
    pub evidence: Option<DiffEvidenceChange<'a>>,
    pub metadata: Option<DiffMetadataChange<'a>>,
    pub requirement: Option<DiffRequirementChange<'a>>,
    pub policy_status: Option<DiffPolicyStatusChange<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffExceptionIdentityChange<'a> {
    pub field: &'a str,
    pub before: Option<&'a str>,
    pub after: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffSelectorIdentityChange<'a> {
    pub changed_fields: &'a [&'a str],
}

#[derive(Debug, Clone, Copy)]
pub struct DiffSelectorPrecisionChange<'a> {
    pub before: u32,
    pub after: u32,
    pub removed_fields: &'a [&'a str],
    pub added_fields: &'a [&'a str],
}

#[derive(Debug, Clone, Copy)]
pub struct DiffScopeChange<'a> {
    pub field: &'a str,
    pub before: Option<&'a str>,
    pub after: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffOccurrenceLimitChange {
    pub before: Option<u32>,
    pub after: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffLifecycleChange<'a> {
    pub field: &'a str,
    pub before: Option<&'a str>,
    pub after: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffEvidenceChange<'a> {
    pub field: &'a str,
    pub removed: &'a [String],
    pub added: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct DiffMetadataChange<'a> {
    pub field: &'a str,
    pub before: Option<&'a str>,
    pub after: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffRequirementChange<'a> {
    pub field: &'a str,
    pub before: bool,
    pub after: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffPolicyStatusChange<'a> {
    pub before: Option<&'a str>,
    pub after: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffReport<'a> {
    pub net_posture: &'a str,
    pub reviewer_action: &'a str,
    pub summary: DiffPostureSummary,
    pub finding_changes: &'a [DiffFindingChange<'a>],
    pub policy_changes: &'a [DiffPolicyChange<'a>],
}
