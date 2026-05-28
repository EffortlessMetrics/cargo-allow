use super::MigrateContext;
use allow_core::{AllowConfig, FindingKind};
use std::path::Path;

pub(super) fn render_migrate_summary(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let output = output.display().to_string();
    allow_report::render_migrate_human(migrate_report(cfg, context, &output, force))
}

pub(super) fn render_migrate_summary_json(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let output = output.display().to_string();
    allow_report::render_migrate_json(migrate_report(cfg, context, &output, force))
}

fn migrate_report<'a>(
    cfg: &'a AllowConfig,
    context: &'a MigrateContext,
    output: &'a str,
    force: bool,
) -> allow_report::MigrateReport<'a> {
    let counts = migrate_summary_counts(cfg);
    let notes = allow_policy_legacy::migration_notes();
    allow_report::MigrateReport {
        inventory: allow_report::InventoryContext::new(
            "source_tree",
            "policy_migration",
            &context.inventory_source,
            context.source_tree_root.as_deref(),
            context.inventory_files,
        ),
        input_kind: &context.input_kind,
        input_path: &context.input_path,
        output_path: output,
        force,
        allow_entries: counts.allow_entries,
        baseline_debt: counts.baseline_debt,
        unsafe_entries: counts.unsafe_entries,
        entries_with_evidence: counts.entries_with_evidence,
        notes,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MigrateSummaryCounts {
    allow_entries: usize,
    baseline_debt: usize,
    unsafe_entries: usize,
    entries_with_evidence: usize,
}

fn migrate_summary_counts(cfg: &AllowConfig) -> MigrateSummaryCounts {
    MigrateSummaryCounts {
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
