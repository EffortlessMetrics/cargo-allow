use crate::non_rust::{render_non_rust_human, render_non_rust_markdown};
use crate::text::markdown_inline_code;
use crate::{
    AUDIT_REVIEW_QUEUE_STATUSES, CLAIM_BOUNDARY_TEXT, ReportContext, ReviewSignals,
    STATUS_COUNT_ORDER, Summary, baseline_debt_count, broken_evidence_link_count,
    policy_missing_evidence_count, render_source_inventory_human, render_source_inventory_markdown,
    weak_evidence_reference_count,
};
use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape};

const HUMAN_NON_MATCHED_OUTCOME_LIMIT: usize = 80;
const MARKDOWN_NON_MATCHED_OUTCOME_LIMIT: usize = 100;
const AUDIT_REVIEW_QUEUE_LIMIT: usize = 20;

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
        context.inventory.source,
        inventory_files_suffix(context)
    ));
    if let Some(root) = context.inventory.root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    for status in STATUS_COUNT_ORDER {
        let count = summary.count(status);
        if count > 0 {
            out.push_str(&format!("  {:24} {}\n", status.as_str(), count));
        }
    }
    if let Some(baseline_debt) = policy_baseline_debt_note(&summary, context) {
        out.push_str(&format!(
            "  {:24} {}\n",
            "policy_baseline_debt", baseline_debt
        ));
    }
    if let Some(policy_missing_evidence) = policy_missing_evidence_note(&summary, context) {
        out.push_str(&format!(
            "  {:24} {}\n",
            "policy_missing_evidence", policy_missing_evidence
        ));
    }
    let broken_evidence_links = broken_evidence_link_count(context);
    if broken_evidence_links > 0 {
        out.push_str(&format!(
            "  {:24} {}\n",
            "broken_evidence_links", broken_evidence_links
        ));
    }
    let weak_evidence_references = weak_evidence_reference_count(context);
    if weak_evidence_references > 0 {
        out.push_str(&format!(
            "  {:24} {}\n",
            "weak_evidence_references", weak_evidence_references
        ));
    }
    if outcomes.is_empty() {
        out.push_str("  no outcomes\n");
    }
    if command == "audit" {
        render_source_inventory_human(findings, outcomes, &mut out);
        render_audit_summary_human(&summary, outcomes, context, &mut out);
    }
    render_non_rust_human(findings, outcomes, &mut out);
    out.push('\n');
    let non_matched = outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    for outcome in non_matched.iter().take(HUMAN_NON_MATCHED_OUTCOME_LIMIT) {
        out.push_str(&format!(
            "{}: {}\n",
            outcome.status.as_str(),
            outcome.message
        ));
    }
    append_human_omitted_outcome_note(&mut out, non_matched.len());
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

fn render_audit_summary_human(
    summary: &Summary,
    outcomes: &[MatchOutcome],
    context: ReportContext<'_>,
    out: &mut String,
) {
    let signals = ReviewSignals::from_summary(summary, context);
    let queue = outcomes
        .iter()
        .filter(|outcome| AUDIT_REVIEW_QUEUE_STATUSES.contains(&outcome.status))
        .collect::<Vec<_>>();
    out.push_str("\nAudit summary:\n");
    out.push_str(&format!("  {:24} {}\n", "match_outcomes", summary.total));
    out.push_str(&format!(
        "  {:24} {}\n",
        "review_items", signals.review_items
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "new_unreceipted",
        summary.count(MatchStatus::New)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "expired",
        summary.count(MatchStatus::Expired)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "evidence_gaps",
        summary.count(MatchStatus::EvidenceMissing)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "policy_missing_evidence", signals.policy_missing_evidence
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "broken_evidence_links", signals.broken_evidence_links
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "weak_evidence_references", signals.weak_evidence_references
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "baseline_debt", signals.baseline_debt
    ));
    out.push_str(audit_recommended_next_step(
        summary,
        signals,
        queue.is_empty(),
    ));
    if !queue.is_empty() {
        out.push_str("\nAudit review queue:\n");
        for outcome in queue.iter().take(AUDIT_REVIEW_QUEUE_LIMIT) {
            out.push_str(&format!(
                "  {}: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
        append_human_omitted_review_queue_note(out, queue.len());
    }
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
        json_escape(context.inventory.source),
        inventory_files_markdown_suffix(context)
    ));
    if let Some(root) = context.inventory.root {
        out.push_str(&format!(
            "Source tree root: `{}`\n\n",
            markdown_inline_code(root)
        ));
    }
    out.push_str("| Status | Count |\n|---|---:|\n");
    for status in STATUS_COUNT_ORDER {
        let count = summary.count(status);
        out.push_str(&format!("| `{}` | {} |\n", status.as_str(), count));
    }
    if let Some(baseline_debt) = policy_baseline_debt_note(&summary, context) {
        out.push_str(&format!("| `policy_baseline_debt` | {} |\n", baseline_debt));
    }
    if let Some(policy_missing_evidence) = policy_missing_evidence_note(&summary, context) {
        out.push_str(&format!(
            "| `policy_missing_evidence` | {} |\n",
            policy_missing_evidence
        ));
    }
    let broken_evidence_links = broken_evidence_link_count(context);
    if broken_evidence_links > 0 {
        out.push_str(&format!(
            "| `broken_evidence_links` | {} |\n",
            broken_evidence_links
        ));
    }
    let weak_evidence_references = weak_evidence_reference_count(context);
    if weak_evidence_references > 0 {
        out.push_str(&format!(
            "| `weak_evidence_references` | {} |\n",
            weak_evidence_references
        ));
    }
    if command == "audit" {
        render_source_inventory_markdown(findings, outcomes, &mut out);
        render_audit_summary_markdown(&summary, outcomes, context, &mut out);
    }
    render_non_rust_markdown(findings, outcomes, &mut out);
    let non_matched = outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    if !non_matched.is_empty() {
        out.push_str("\n## Non-matched outcomes\n\n");
        for outcome in non_matched.iter().take(MARKDOWN_NON_MATCHED_OUTCOME_LIMIT) {
            out.push_str(&format!(
                "- `{}`: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
        append_markdown_omitted_outcome_note(&mut out, non_matched.len());
    }
    out.push_str("\n> ");
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn append_human_omitted_outcome_note(out: &mut String, outcome_count: usize) {
    if outcome_count > HUMAN_NON_MATCHED_OUTCOME_LIMIT {
        let omitted = outcome_count - HUMAN_NON_MATCHED_OUTCOME_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "... {omitted} additional non-matched outcome{plural} omitted from this listing\n"
        ));
    }
}

fn append_markdown_omitted_outcome_note(out: &mut String, outcome_count: usize) {
    if outcome_count > MARKDOWN_NON_MATCHED_OUTCOME_LIMIT {
        let omitted = outcome_count - MARKDOWN_NON_MATCHED_OUTCOME_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "\n{omitted} additional non-matched outcome{plural} omitted from this listing.\n"
        ));
    }
}

fn render_audit_summary_markdown(
    summary: &Summary,
    outcomes: &[MatchOutcome],
    context: ReportContext<'_>,
    out: &mut String,
) {
    let signals = ReviewSignals::from_summary(summary, context);
    let queue = outcomes
        .iter()
        .filter(|outcome| AUDIT_REVIEW_QUEUE_STATUSES.contains(&outcome.status))
        .collect::<Vec<_>>();
    out.push_str("\n## Audit Summary\n\n");
    out.push_str("| Signal | Count |\n|---|---:|\n");
    out.push_str(&format!("| Match outcomes | {} |\n", summary.total));
    out.push_str(&format!("| Review items | {} |\n", signals.review_items));
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
    out.push_str(&format!(
        "| Policy missing evidence | {} |\n",
        signals.policy_missing_evidence
    ));
    out.push_str(&format!(
        "| Broken evidence links | {} |\n",
        signals.broken_evidence_links
    ));
    out.push_str(&format!(
        "| Weak evidence/link references | {} |\n",
        signals.weak_evidence_references
    ));
    out.push_str(&format!("| Baseline debt | {} |\n", signals.baseline_debt));
    out.push_str(audit_recommended_next_step(
        summary,
        signals,
        queue.is_empty(),
    ));

    if !queue.is_empty() {
        out.push_str("\n## Audit Review Queue\n\n");
        for outcome in queue.iter().take(AUDIT_REVIEW_QUEUE_LIMIT) {
            out.push_str(&format!(
                "- `{}`: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
        append_markdown_omitted_review_queue_note(out, queue.len());
    }
}

fn append_human_omitted_review_queue_note(out: &mut String, queue_count: usize) {
    if queue_count > AUDIT_REVIEW_QUEUE_LIMIT {
        let omitted = queue_count - AUDIT_REVIEW_QUEUE_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "  ... {omitted} additional audit review item{plural} omitted from this queue\n"
        ));
    }
}

fn append_markdown_omitted_review_queue_note(out: &mut String, queue_count: usize) {
    if queue_count > AUDIT_REVIEW_QUEUE_LIMIT {
        let omitted = queue_count - AUDIT_REVIEW_QUEUE_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "\n{omitted} additional audit review item{plural} omitted from this queue.\n"
        ));
    }
}

fn audit_recommended_next_step(
    summary: &Summary,
    signals: ReviewSignals,
    queue_empty: bool,
) -> &'static str {
    if signals.review_items == 0 {
        "\nRecommended next step: keep `cargo-allow check --mode no-new` in CI.\n"
    } else if queue_empty && signals.broken_evidence_links > 0 {
        "\nRecommended next step: run `cargo-allow worklist --item-kind broken_evidence_link --format json` to repair broken local evidence/link references.\n"
    } else if queue_empty
        && signals.policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing)
    {
        "\nRecommended next step: run `cargo-allow worklist --missing-evidence --format json` to route retained entries with no evidence references.\n"
    } else if queue_empty && signals.weak_evidence_references > 0 {
        "\nRecommended next step: replace unstructured or unknown-prefix evidence/link references with recognized prefixes before tightening policy.\n"
    } else if queue_empty && signals.baseline_debt > 0 {
        "\nRecommended next step: run `cargo-allow worklist --format json` to review generated baseline debt.\n"
    } else {
        "\nRecommended next step: review the queue below before tightening policy.\n"
    }
}

fn inventory_files_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory
        .files_scanned
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
}

fn inventory_files_markdown_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory
        .files_scanned
        .map(|files| format!("; files scanned: `{files}`"))
        .unwrap_or_default()
}

fn policy_baseline_debt_note(summary: &Summary, context: ReportContext<'_>) -> Option<usize> {
    let baseline_debt = baseline_debt_count(summary, context);
    (baseline_debt > summary.count(MatchStatus::BaselineDebt)).then_some(baseline_debt)
}

fn policy_missing_evidence_note(summary: &Summary, context: ReportContext<'_>) -> Option<usize> {
    let policy_missing_evidence = policy_missing_evidence_count(summary, context);
    (policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing))
        .then_some(policy_missing_evidence)
}
