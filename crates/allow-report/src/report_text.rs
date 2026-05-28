use crate::non_rust::{render_non_rust_human, render_non_rust_markdown};
use crate::text::markdown_inline_code;
use crate::{
    CLAIM_BOUNDARY_TEXT, ReportContext, Summary, baseline_debt_count,
    review_item_count_with_baseline,
};
use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape};

pub fn render_human(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_human_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_human_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str(&format!("cargo-allow {command}\n\n"));
    out.push_str(&format!("Findings scanned: {}\n", findings.len()));
    out.push_str(&format!(
        "Inventory: source_tree/source_syntax via {}{}\n",
        context.inventory_source,
        inventory_files_suffix(context)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
    ] {
        let count = summary.count(status);
        if count > 0 {
            out.push_str(&format!("  {:24} {}\n", status.as_str(), count));
        }
    }
    if outcomes.is_empty() {
        out.push_str("  no outcomes\n");
    }
    render_non_rust_human(findings, outcomes, &mut out);
    out.push('\n');
    for outcome in outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .take(80)
    {
        out.push_str(&format!(
            "{}: {}\n",
            outcome.status.as_str(),
            outcome.message
        ));
    }
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out.push_str(if failed {
        "Result: failed\n"
    } else {
        "Result: passed/advisory\n"
    });
    out
}

pub fn render_markdown(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_markdown_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_markdown_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str(&format!("# cargo-allow {command}\n\n"));
    out.push_str(&format!(
        "**Result:** {}\n\n",
        if failed { "failed" } else { "passed/advisory" }
    ));
    out.push_str(&format!("Findings scanned: `{}`\n\n", findings.len()));
    out.push_str(&format!(
        "Inventory: `source_tree` / `source_syntax` via `{}`{}\n\n",
        json_escape(context.inventory_source),
        inventory_files_markdown_suffix(context)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(
            "Source tree root: `{}`\n\n",
            markdown_inline_code(root)
        ));
    }
    out.push_str("| Status | Count |\n|---|---:|\n");
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
    ] {
        let count = summary.count(status);
        out.push_str(&format!("| `{}` | {} |\n", status.as_str(), count));
    }
    if command == "audit" {
        render_audit_summary_markdown(&summary, outcomes, context, &mut out);
    }
    render_non_rust_markdown(findings, outcomes, &mut out);
    let non_matched = outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .take(100)
        .collect::<Vec<_>>();
    if !non_matched.is_empty() {
        out.push_str("\n## Non-matched outcomes\n\n");
        for outcome in non_matched {
            out.push_str(&format!(
                "- `{}`: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
    }
    out.push_str("\n> ");
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn render_audit_summary_markdown(
    summary: &Summary,
    outcomes: &[MatchOutcome],
    context: ReportContext<'_>,
    out: &mut String,
) {
    let review_statuses = [
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
        MatchStatus::Stale,
        MatchStatus::ReviewDue,
    ];
    let baseline_debt = baseline_debt_count(summary, context);
    let review_items = review_item_count_with_baseline(summary, baseline_debt);
    let queue = outcomes
        .iter()
        .filter(|outcome| review_statuses.contains(&outcome.status))
        .take(20)
        .collect::<Vec<_>>();
    out.push_str("\n## Audit Summary\n\n");
    out.push_str("| Signal | Count |\n|---|---:|\n");
    out.push_str(&format!("| Match outcomes | {} |\n", summary.total));
    out.push_str(&format!("| Review items | {} |\n", review_items));
    out.push_str(&format!(
        "| New unreceipted | {} |\n",
        summary.count(MatchStatus::New)
    ));
    out.push_str(&format!(
        "| Expired | {} |\n",
        summary.count(MatchStatus::Expired)
    ));
    out.push_str(&format!(
        "| Evidence gaps | {} |\n",
        summary.count(MatchStatus::EvidenceMissing)
    ));
    out.push_str(&format!("| Baseline debt | {} |\n", baseline_debt));
    if review_items == 0 {
        out.push_str("\nRecommended next step: keep `cargo-allow check --mode no-new` in CI.\n");
    } else if queue.is_empty() && baseline_debt > 0 {
        out.push_str("\nRecommended next step: run `cargo-allow worklist --format json` to review generated baseline debt.\n");
    } else {
        out.push_str("\nRecommended next step: review the queue below before tightening policy.\n");
    }

    if !queue.is_empty() {
        out.push_str("\n## Audit Review Queue\n\n");
        for outcome in queue {
            out.push_str(&format!(
                "- `{}`: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
    }
}

fn inventory_files_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory_files
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
}

fn inventory_files_markdown_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory_files
        .map(|files| format!("; files scanned: `{files}`"))
        .unwrap_or_default()
}
