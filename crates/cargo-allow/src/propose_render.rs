use super::ProposeContext;
use std::path::Path;

pub(super) fn render_propose_summary(
    findings: usize,
    proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    allow_report::render_propose_human(propose_report(
        findings,
        proposed_entries,
        expires,
        output_text.as_deref(),
        false,
        ProposeContext::default(),
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
    allow_report::render_propose_json(propose_report(
        findings,
        proposed_entries,
        expires,
        output_text.as_deref(),
        force,
        context,
    ))
}

fn propose_report<'a>(
    findings: usize,
    proposed_entries: usize,
    expires: &'a str,
    policy_output: Option<&'a str>,
    force: bool,
    context: ProposeContext<'a>,
) -> allow_report::ProposeReport<'a> {
    allow_report::ProposeReport {
        inventory: context.inventory,
        kind: context.kind_filter,
        expires,
        policy_output,
        force,
        findings_scanned: findings,
        baseline_debt_entries_proposed: proposed_entries,
    }
}
