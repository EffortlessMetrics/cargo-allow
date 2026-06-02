use super::MigrateContext;
use allow_core::{AllowConfig, FindingKind};
use std::path::{Path, PathBuf};

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
    let notes = allow_policy_legacy::migration_notes();
    let mut report = allow_report::MigrateReport::from_config(
        allow_report::InventoryContext::policy_migration(
            &context.inventory_source,
            context.source_tree_root.as_deref(),
            context.inventory_files,
        ),
        cfg,
        &context.input_kind,
        &context.input_path,
        output,
        force,
        notes,
    );
    let evidence_root = evidence_diagnostic_root(context);
    let broken_evidence_links =
        allow_policy::broken_evidence_link_count(evidence_root.as_path(), cfg);
    report.broken_evidence_links = (broken_evidence_links > 0).then_some(broken_evidence_links);
    let unsafe_broken_evidence_links =
        unsafe_broken_evidence_link_count(evidence_root.as_path(), cfg);
    report.unsafe_broken_evidence_links =
        (unsafe_broken_evidence_links > 0).then_some(unsafe_broken_evidence_links);
    let weak_evidence_references =
        allow_policy::weak_evidence_reference_count(evidence_root.as_path(), cfg);
    report.weak_evidence_references =
        (weak_evidence_references > 0).then_some(weak_evidence_references);
    let unsafe_weak_evidence_references =
        unsafe_weak_evidence_reference_count(evidence_root.as_path(), cfg);
    report.unsafe_weak_evidence_references =
        (unsafe_weak_evidence_references > 0).then_some(unsafe_weak_evidence_references);
    report
}

fn evidence_diagnostic_root(context: &MigrateContext) -> PathBuf {
    context
        .source_tree_root
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn unsafe_weak_evidence_reference_count(root: &Path, cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.kind == FindingKind::Unsafe)
        .flat_map(|entry| allow_policy::policy_reference_diagnostics(root, entry))
        .filter(|reference| reference.diagnostic.status.is_weak_reference())
        .count()
}

fn unsafe_broken_evidence_link_count(root: &Path, cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.kind == FindingKind::Unsafe)
        .flat_map(|entry| allow_policy::policy_reference_diagnostics(root, entry))
        .filter(|reference| reference.diagnostic.status.is_broken_local_link())
        .count()
}
