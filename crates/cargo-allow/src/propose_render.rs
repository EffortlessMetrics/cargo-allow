use super::ProposeContext;
use std::path::Path;

/// Bundled propose summary counts to keep the render function signatures
/// under clippy's argument-count threshold.
pub(super) struct ProposeCounts {
    pub(super) findings_scanned: usize,
    pub(super) proposed_entries: usize,
    pub(super) unsafe_proposed_entries: usize,
    pub(super) truncated_new_findings: usize,
}

#[cfg(test)]
pub(super) fn render_propose_summary(
    counts: ProposeCounts,
    expires: &str,
    output: Option<&Path>,
    context: ProposeContext<'_>,
) -> String {
    render_propose_summary_styled(counts, expires, output, context, allow_report::Style::PLAIN)
}

pub(super) fn render_propose_summary_styled(
    counts: ProposeCounts,
    expires: &str,
    output: Option<&Path>,
    context: ProposeContext<'_>,
    style: allow_report::Style,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    allow_report::render_propose_human_styled(
        allow_report::ProposeReport {
            inventory: context.inventory,
            kind: context.kind_filter,
            expires,
            policy_output: output_text.as_deref(),
            force: false,
            findings_scanned: counts.findings_scanned,
            baseline_debt_entries_proposed: counts.proposed_entries,
            unsafe_baseline_debt_entries_proposed: counts.unsafe_proposed_entries,
            truncated_new_findings: counts.truncated_new_findings,
            mutation_receipt: context.mutation_receipt,
        },
        style,
    )
}

pub(super) fn render_propose_summary_json(
    counts: ProposeCounts,
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
        findings_scanned: counts.findings_scanned,
        baseline_debt_entries_proposed: counts.proposed_entries,
        unsafe_baseline_debt_entries_proposed: counts.unsafe_proposed_entries,
        truncated_new_findings: counts.truncated_new_findings,
        mutation_receipt: context.mutation_receipt,
    })
}
