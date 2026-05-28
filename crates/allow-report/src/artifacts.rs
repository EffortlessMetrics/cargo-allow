use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome};

use crate::InventoryContext;

#[derive(Debug, Clone, Copy)]
pub struct PruneModeContext<'a> {
    pub explicit_dry_run: bool,
    pub write_requested: bool,
    pub written_path: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct PruneCandidate<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub owner: &'a str,
    pub classification: &'a str,
    pub scope: &'a str,
    pub reason: &'a str,
}

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

#[derive(Debug, Clone, Copy, Default)]
pub struct EvidenceReference<'a> {
    pub raw: &'a str,
    pub prefix: Option<&'a str>,
    pub target: Option<&'a str>,
    pub status: &'a str,
    pub message: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ExplainReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub current_findings: &'a [Finding],
    pub match_outcomes: &'a [MatchOutcome],
    pub evidence_references: &'a [EvidenceReference<'a>],
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorklistFilters<'a> {
    pub kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub item_kind: Option<&'a str>,
    pub status: Option<&'a str>,
    pub allow_id: Option<&'a str>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub baseline_debt: bool,
    pub broad_scope: bool,
    pub risk: Option<&'a str>,
    pub difficulty: Option<&'a str>,
    pub missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct WorklistItem<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub exception_kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub created: Option<&'a str>,
    pub review_after: Option<&'a str>,
    pub expires: Option<&'a str>,
    pub evidence_count: Option<usize>,
    pub risk: &'a str,
    pub difficulty: &'a str,
    pub status: &'a str,
    pub allow_id: Option<&'a str>,
    pub finding_index: Option<usize>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub message: &'a str,
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct DoctorReport<'a> {
    pub source_tree_root: &'a str,
    pub root_discovery: &'a str,
    pub config_path: Option<&'a str>,
    pub inventory_source: &'a str,
    pub files_scanned: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ProposeReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub kind: Option<&'a str>,
    pub expires: &'a str,
    pub policy_output: Option<&'a str>,
    pub force: bool,
    pub findings_scanned: usize,
    pub baseline_debt_entries_proposed: usize,
}

impl<'a> ProposeReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        kind: Option<&'a str>,
        expires: &'a str,
        policy_output: Option<&'a str>,
        force: bool,
        findings_scanned: usize,
        baseline_debt_entries_proposed: usize,
    ) -> Self {
        Self {
            inventory,
            kind,
            expires,
            policy_output,
            force,
            findings_scanned,
            baseline_debt_entries_proposed,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AddReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub selected_finding: &'a Finding,
    pub policy_output: Option<&'a str>,
    pub force: bool,
}

impl<'a> AddReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        entry: &'a AllowEntry,
        selected_finding: &'a Finding,
        policy_output: Option<&'a str>,
        force: bool,
    ) -> Self {
        Self {
            inventory,
            entry,
            selected_finding,
            policy_output,
            force,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MigrateReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub input_kind: &'a str,
    pub input_path: &'a str,
    pub output_path: &'a str,
    pub force: bool,
    pub allow_entries: usize,
    pub baseline_debt: usize,
    pub unsafe_entries: usize,
    pub entries_with_evidence: usize,
    pub notes: &'a str,
}

impl<'a> MigrateReport<'a> {
    pub fn from_config(
        inventory: InventoryContext<'a>,
        cfg: &AllowConfig,
        input_kind: &'a str,
        input_path: &'a str,
        output_path: &'a str,
        force: bool,
        notes: &'a str,
    ) -> Self {
        let counts = MigrateSummaryCounts::from_config(cfg);
        Self {
            inventory,
            input_kind,
            input_path,
            output_path,
            force,
            allow_entries: counts.allow_entries,
            baseline_debt: counts.baseline_debt,
            unsafe_entries: counts.unsafe_entries,
            entries_with_evidence: counts.entries_with_evidence,
            notes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MigrateSummaryCounts {
    allow_entries: usize,
    baseline_debt: usize,
    unsafe_entries: usize,
    entries_with_evidence: usize,
}

impl MigrateSummaryCounts {
    fn from_config(cfg: &AllowConfig) -> Self {
        Self {
            allow_entries: cfg.allow.len(),
            baseline_debt: cfg
                .allow
                .iter()
                .filter(|entry| entry.classification == "baseline_debt")
                .count(),
            unsafe_entries: cfg
                .allow
                .iter()
                .filter(|entry| entry.kind == FindingKind::Unsafe)
                .count(),
            entries_with_evidence: cfg
                .allow
                .iter()
                .filter(|entry| !entry.evidence.is_empty())
                .count(),
        }
    }
}

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
}

#[derive(Debug, Clone, Copy)]
pub struct DiffReport<'a> {
    pub net_posture: &'a str,
    pub reviewer_action: &'a str,
    pub summary: DiffPostureSummary,
    pub finding_changes: &'a [DiffFindingChange<'a>],
    pub policy_changes: &'a [DiffPolicyChange<'a>],
}
