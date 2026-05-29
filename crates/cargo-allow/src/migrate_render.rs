use super::MigrateContext;
use allow_core::AllowConfig;
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
    let notes = allow_policy_legacy::migration_notes();
    allow_report::MigrateReport::from_config(
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
    )
}
