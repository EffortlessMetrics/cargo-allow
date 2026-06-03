use allow_core::{AllowConfig, FindingKind};

use crate::InventoryContext;

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
    pub lint_exception_entries: usize,
    pub entries_with_evidence: usize,
    pub evidence_entries: usize,
    pub broken_evidence_links: Option<usize>,
    pub unsafe_broken_evidence_links: Option<usize>,
    pub weak_evidence_references: Option<usize>,
    pub unsafe_weak_evidence_references: Option<usize>,
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
            lint_exception_entries: counts.lint_exception_entries,
            entries_with_evidence: counts.entries_with_evidence,
            evidence_entries: counts.evidence_entries,
            broken_evidence_links: None,
            unsafe_broken_evidence_links: None,
            weak_evidence_references: None,
            unsafe_weak_evidence_references: None,
            notes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MigrateSummaryCounts {
    allow_entries: usize,
    baseline_debt: usize,
    unsafe_entries: usize,
    lint_exception_entries: usize,
    entries_with_evidence: usize,
    evidence_entries: usize,
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
            lint_exception_entries: cfg
                .allow
                .iter()
                .filter(|entry| entry.kind == FindingKind::LintException)
                .count(),
            entries_with_evidence: cfg
                .allow
                .iter()
                .filter(|entry| !entry.evidence.is_empty())
                .count(),
            evidence_entries: cfg.allow.iter().map(|entry| entry.evidence.len()).sum(),
        }
    }
}
