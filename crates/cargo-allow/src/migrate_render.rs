use super::MigrateContext;
use allow_core::{AllowConfig, FindingKind};
use std::path::Path;

pub(super) fn render_migrate_summary(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let counts = migrate_summary_counts(cfg);
    let notes = allow_policy_legacy::migration_notes();
    let mut out = String::new();
    out.push_str("cargo-allow migrate summary\n");
    out.push_str(&format!("input_kind: {}\n", context.input_kind));
    out.push_str(&format!("input: {}\n", context.input_path));
    out.push_str(&format!("output: {}\n", output.display()));
    out.push_str(&format!("force: {force}\n"));
    out.push_str(&format!("allow_entries: {}\n", counts.allow_entries));
    out.push_str(&format!("baseline_debt: {}\n", counts.baseline_debt));
    out.push_str(&format!("unsafe_entries: {}\n", counts.unsafe_entries));
    if let Some(root) = &context.source_tree_root {
        out.push_str(&format!("source_tree_root: {root}\n"));
    }
    out.push_str(&format!("inventory_source: {}\n", context.inventory_source));
    if let Some(files) = context.inventory_files {
        out.push_str(&format!("files_scanned: {files}\n"));
    }
    out.push_str(notes);
    out
}

pub(super) fn render_migrate_summary_json(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let counts = migrate_summary_counts(cfg);
    let output = output.display().to_string();
    let notes = allow_policy_legacy::migration_notes();
    allow_report::render_migrate_json(allow_report::MigrateReport {
        inventory: allow_report::InventoryContext::new(
            "source_tree",
            "policy_migration",
            &context.inventory_source,
            context.source_tree_root.as_deref(),
            context.inventory_files,
        ),
        input_kind: &context.input_kind,
        input_path: &context.input_path,
        output_path: &output,
        force,
        allow_entries: counts.allow_entries,
        baseline_debt: counts.baseline_debt,
        unsafe_entries: counts.unsafe_entries,
        entries_with_evidence: counts.entries_with_evidence,
        notes,
    })
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
