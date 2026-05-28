use allow_core::{AllowEntry, Finding};
use std::path::Path;

use super::AddContext;
use crate::source_syntax_inventory_context;

pub(super) fn render_add_summary(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    allow_report::render_add_human(add_report(
        entry,
        finding,
        policy_output.as_deref(),
        false,
        AddContext::default(),
    ))
}

pub(super) fn render_add_summary_json(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    force: bool,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    allow_report::render_add_json(add_report(
        entry,
        finding,
        policy_output.as_deref(),
        force,
        context,
    ))
}

fn add_report<'a>(
    entry: &'a AllowEntry,
    finding: &'a Finding,
    policy_output: Option<&'a str>,
    force: bool,
    context: AddContext<'a>,
) -> allow_report::AddReport<'a> {
    allow_report::AddReport {
        inventory: source_syntax_inventory_context(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
        entry,
        selected_finding: finding,
        policy_output,
        force,
    }
}
