use super::ProposeContext;
use std::path::Path;

pub(super) fn render_propose_summary(
    findings: usize,
    proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    let context = ProposeContext::default();
    allow_report::render_propose_human(allow_report::ProposeReport::new(
        context.inventory,
        context.kind_filter,
        expires,
        output_text.as_deref(),
        false,
        findings,
        proposed_entries,
    ))
}

pub(super) fn render_propose_summary_json(
    findings: usize,
    proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
    force: bool,
    context: ProposeContext<'_>,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    allow_report::render_propose_json(allow_report::ProposeReport::new(
        context.inventory,
        context.kind_filter,
        expires,
        output_text.as_deref(),
        force,
        findings,
        proposed_entries,
    ))
}
