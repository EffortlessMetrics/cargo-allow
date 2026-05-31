use allow_core::{AllowEntry, Finding};
use std::path::Path;

use super::AddContext;

pub(super) fn render_add_summary(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    allow_report::render_add_human(allow_report::AddReport::new(
        context.inventory,
        entry,
        finding,
        policy_output.as_deref(),
        false,
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
    allow_report::render_add_json(allow_report::AddReport::new(
        context.inventory,
        entry,
        finding,
        policy_output.as_deref(),
        force,
    ))
}
