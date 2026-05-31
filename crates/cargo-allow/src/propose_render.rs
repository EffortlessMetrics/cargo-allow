use super::ProposeContext;
use std::path::Path;

pub(super) fn render_propose_summary(
    findings: usize,
    proposed_entries: usize,
    unsafe_proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    let context = ProposeContext::default();
    allow_report::render_propose_human(allow_report::ProposeReport {
        inventory: context.inventory,
        kind: context.kind_filter,
        expires,
        policy_output: output_text.as_deref(),
        force: false,
        findings_scanned: findings,
        baseline_debt_entries_proposed: proposed_entries,
        unsafe_baseline_debt_entries_proposed: unsafe_proposed_entries,
    })
}

pub(super) fn render_propose_summary_json(
    findings: usize,
    proposed_entries: usize,
    unsafe_proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
    force: bool,
    context: ProposeContext<'_>,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    allow_report::render_propose_json(allow_report::ProposeReport {
        inventory: context.inventory,
        kind: context.kind_filter,
        expires,
        policy_output: output_text.as_deref(),
        force,
        findings_scanned: findings,
        baseline_debt_entries_proposed: proposed_entries,
        unsafe_baseline_debt_entries_proposed: unsafe_proposed_entries,
    })
}
