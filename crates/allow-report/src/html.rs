use allow_core::{Finding, MatchOutcome, MatchStatus};

use crate::audit_remediation::audit_remediation_items;
use crate::evidence_repair::{
    BROKEN_EVIDENCE_LINK_COMMAND, WEAK_EVIDENCE_REFERENCE_COMMAND, evidence_repair_queues,
};
use crate::text::html_escape;
use crate::{
    CLAIM_BOUNDARY_TEXT, FilePosture, ReportContext, ReviewSignals, STATUS_COUNT_ORDER, Summary,
    audit_review_queue, baseline_debt_count, broken_evidence_link_count, non_rust_file_rows,
    policy_missing_evidence_count, render_source_inventory_html, weak_evidence_reference_count,
};

pub fn render_html(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_html_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_html_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"utf-8\">\n");
    out.push_str(&format!(
        "  <title>cargo-allow {}</title>\n",
        html_escape(command)
    ));
    out.push_str("  <style>body{font-family:system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;line-height:1.45}table{border-collapse:collapse;width:100%;margin:1rem 0}th,td{border:1px solid #d0d7de;padding:.4rem .55rem;text-align:left}th{background:#f6f8fa}td.count{text-align:right;font-variant-numeric:tabular-nums}.status{font-weight:700}.failed{color:#b42318}.passed{color:#1a7f37}code{background:#f6f8fa;padding:.1rem .25rem;border-radius:4px}.claim{border-left:4px solid #57606a;padding-left:1rem;color:#57606a}</style>\n");
    out.push_str("</head>\n<body>\n");
    out.push_str(&format!("<h1>cargo-allow {}</h1>\n", html_escape(command)));
    out.push_str(&format!(
        "<p class=\"status {}\">Result: {}</p>\n",
        if failed { "failed" } else { "passed" },
        if failed {
            "failed"
        } else {
            context.enforcement.unwrap_or("advisory")
        }
    ));
    out.push_str(&format!(
        "<p>Findings scanned: <code>{}</code></p>\n",
        findings.len()
    ));
    out.push_str(&format!(
        "<p>Inventory: <code>source_tree</code> / <code>source_syntax</code> via <code>{}</code>{}</p>\n",
        html_escape(context.inventory.source),
        inventory_files_html_suffix(context)
    ));
    if let Some(root) = context.inventory.root {
        out.push_str(&format!(
            "<p>Source tree root: <code>{}</code></p>\n",
            html_escape(root)
        ));
    }
    out.push_str("<h2>Status Counts</h2>\n");
    render_status_count_table_html(&summary, context, &mut out);
    if command != "audit" {
        let signals = ReviewSignals::from_summary(&summary, context);
        render_evidence_repair_queues_html(&summary, signals, &mut out);
    }
    if command == "audit" {
        render_source_inventory_html(findings, outcomes, &mut out);
        render_audit_summary_html(&summary, outcomes, context, &mut out);
    }
    render_non_rust_html(findings, outcomes, &mut out);
    render_non_matched_html(outcomes, &mut out);
    out.push_str("<h2>Claim Boundary</h2>\n");
    out.push_str(&format!(
        "<p class=\"claim\">{}</p>\n",
        html_escape(CLAIM_BOUNDARY_TEXT)
    ));
    out.push_str("</body>\n</html>\n");
    out
}

fn render_status_count_table_html(summary: &Summary, context: ReportContext<'_>, out: &mut String) {
    out.push_str("<table><thead><tr><th>Status</th><th>Count</th></tr></thead><tbody>\n");
    for status in STATUS_COUNT_ORDER {
        out.push_str(&format!(
            "<tr><td><code>{}</code></td><td class=\"count\">{}</td></tr>\n",
            status.as_str(),
            summary.count(status)
        ));
    }
    for (name, count) in policy_context_count_rows(summary, context) {
        out.push_str(&format!(
            "<tr><td><code>{}</code></td><td class=\"count\">{}</td></tr>\n",
            html_escape(name),
            count
        ));
    }
    out.push_str("</tbody></table>\n");
}

fn policy_context_count_rows(
    summary: &Summary,
    context: ReportContext<'_>,
) -> Vec<(&'static str, usize)> {
    let mut rows = Vec::new();
    let baseline_debt = baseline_debt_count(summary, context);
    if baseline_debt > summary.count(MatchStatus::BaselineDebt) {
        rows.push(("policy_baseline_debt", baseline_debt));
    }
    let policy_missing_evidence = policy_missing_evidence_count(summary, context);
    if policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing) {
        rows.push(("policy_missing_evidence", policy_missing_evidence));
    }
    let broken_evidence_links = broken_evidence_link_count(context);
    if broken_evidence_links > 0 {
        rows.push(("broken_evidence_links", broken_evidence_links));
    }
    let weak_evidence_references = weak_evidence_reference_count(context);
    if weak_evidence_references > 0 {
        rows.push(("weak_evidence_references", weak_evidence_references));
    }
    rows
}

fn render_audit_summary_html(
    summary: &Summary,
    outcomes: &[MatchOutcome],
    context: ReportContext<'_>,
    out: &mut String,
) {
    let signals = ReviewSignals::from_summary(summary, context);
    let queue = audit_review_queue(outcomes);
    out.push_str("<h2>Audit Summary</h2>\n");
    out.push_str("<table><thead><tr><th>Signal</th><th>Count</th></tr></thead><tbody>\n");
    for (name, value) in [
        ("Match outcomes", summary.total),
        ("Review items", signals.review_items),
        ("New unreceipted", summary.count(MatchStatus::New)),
        ("Expired", summary.count(MatchStatus::Expired)),
        ("Review due", summary.count(MatchStatus::ReviewDue)),
        ("Stale", summary.count(MatchStatus::Stale)),
        ("Ambiguous", summary.count(MatchStatus::Ambiguous)),
        (
            "Invalid selectors",
            summary.count(MatchStatus::InvalidSelector),
        ),
        (
            "Missing required fields",
            summary.count(MatchStatus::MissingRequiredField),
        ),
        ("Evidence gaps", summary.count(MatchStatus::EvidenceMissing)),
        ("Policy missing evidence", signals.policy_missing_evidence),
        ("Broken evidence links", signals.broken_evidence_links),
        (
            "Weak evidence/link references",
            signals.weak_evidence_references,
        ),
        ("Baseline debt", signals.baseline_debt),
    ] {
        out.push_str(&format!(
            "<tr><td>{}</td><td class=\"count\">{}</td></tr>\n",
            html_escape(name),
            value
        ));
    }
    out.push_str("</tbody></table>\n");
    if signals.review_items == 0 {
        out.push_str("<p>Recommended next step: keep <code>cargo-allow check --mode no-new</code> in CI.</p>\n");
    } else if queue.is_empty() && signals.broken_evidence_links > 0 {
        out.push_str(&format!("<p>Recommended next step: run <code>{}</code> to repair broken local evidence/link references.</p>\n", html_escape(BROKEN_EVIDENCE_LINK_COMMAND)));
    } else if queue.is_empty()
        && signals.policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing)
    {
        out.push_str("<p>Recommended next step: run <code>cargo-allow worklist --format json</code> to route retained entries with no evidence references; add <code>--missing-evidence</code> to focus that queue.</p>\n");
    } else if queue.is_empty() && signals.weak_evidence_references > 0 {
        out.push_str(&format!("<p>Recommended next step: run <code>{}</code> to replace unstructured or unknown-prefix evidence/link references.</p>\n", html_escape(WEAK_EVIDENCE_REFERENCE_COMMAND)));
    } else if queue.is_empty() && signals.baseline_debt > 0 {
        out.push_str("<p>Recommended next step: run <code>cargo-allow worklist --format json</code> to review generated baseline debt.</p>\n");
    } else {
        out.push_str(
            "<p>Recommended next step: review the queue below before tightening policy.</p>\n",
        );
    }
    render_audit_remediation_roadmap_html(summary, signals, out);
    render_evidence_repair_queues_html(summary, signals, out);
    if !queue.is_empty() {
        out.push_str("<h2>Audit Review Queue</h2>\n<ul>\n");
        for outcome in queue {
            out.push_str(&format!(
                "<li><code>{}</code>: {}</li>\n",
                outcome.status.as_str(),
                html_escape(&outcome.message)
            ));
        }
        out.push_str("</ul>\n");
    }
}

fn render_audit_remediation_roadmap_html(
    summary: &Summary,
    signals: ReviewSignals,
    out: &mut String,
) {
    let items = audit_remediation_items(summary, signals);
    if items.is_empty() {
        return;
    }
    out.push_str("<h2>Audit Remediation Roadmap</h2>\n");
    out.push_str("<table><thead><tr><th>Signal</th><th>Command</th></tr></thead><tbody>\n");
    for item in items {
        out.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td></tr>\n",
            html_escape(item.label),
            html_escape(item.command)
        ));
    }
    out.push_str("</tbody></table>\n");
}

fn render_evidence_repair_queues_html(summary: &Summary, signals: ReviewSignals, out: &mut String) {
    let commands = evidence_repair_commands(summary, signals);
    if commands.is_empty() {
        return;
    }
    out.push_str("<h3>Evidence Repair Queues</h3>\n<ul>\n");
    for command in commands {
        out.push_str(&format!("<li><code>{}</code></li>\n", html_escape(command)));
    }
    out.push_str("</ul>\n");
}

fn evidence_repair_commands(summary: &Summary, signals: ReviewSignals) -> Vec<&'static str> {
    evidence_repair_queues(summary, signals)
        .into_iter()
        .map(|queue| queue.command)
        .collect()
}

fn render_non_rust_html(findings: &[Finding], outcomes: &[MatchOutcome], out: &mut String) {
    let posture = FilePosture::from_report(findings, outcomes);
    if !posture.has_files() {
        return;
    }
    out.push_str("<h2>Non-Rust File Inventory</h2>\n");
    out.push_str("<table><thead><tr><th>Metric</th><th>Count</th></tr></thead><tbody>\n");
    for (name, value) in [
        ("Files scanned", posture.total),
        ("Matched", posture.matched),
        ("New", posture.new),
        ("Generated", posture.generated),
    ] {
        out.push_str(&format!(
            "<tr><td>{}</td><td class=\"count\">{}</td></tr>\n",
            html_escape(name),
            value
        ));
    }
    out.push_str("</tbody></table>\n");
    if !posture.by_family.is_empty() {
        out.push_str("<table><thead><tr><th>Family</th><th>Count</th></tr></thead><tbody>\n");
        for (family, count) in posture.by_family {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td class=\"count\">{}</td></tr>\n",
                html_escape(&family),
                count
            ));
        }
        out.push_str("</tbody></table>\n");
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        let total = rows.len();
        out.push_str(
            "<table><thead><tr><th>Status</th><th>Family</th><th>Path</th></tr></thead><tbody>\n",
        );
        for row in rows.into_iter().take(60) {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td><td><code>{}</code></td></tr>\n",
                html_escape(row.status),
                html_escape(&row.family),
                html_escape(&row.path)
            ));
        }
        out.push_str("</tbody></table>\n");
        if total > 60 {
            out.push_str(&format!(
                "<p class=\"claim\">{} more non-Rust file rows omitted from HTML report.</p>\n",
                total - 60
            ));
        }
    }
}

fn render_non_matched_html(outcomes: &[MatchOutcome], out: &mut String) {
    let non_matched_total = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .count();
    if non_matched_total == 0 {
        return;
    }
    out.push_str("<h2>Non-matched Outcomes</h2>\n<ul>\n");
    for outcome in outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .take(100)
    {
        out.push_str(&format!(
            "<li><code>{}</code>: {}</li>\n",
            outcome.status.as_str(),
            html_escape(&outcome.message)
        ));
    }
    out.push_str("</ul>\n");
    if non_matched_total > 100 {
        out.push_str(&format!(
            "<p class=\"claim\">{} more non-matched outcomes omitted from HTML report.</p>\n",
            non_matched_total - 100
        ));
    }
}

fn inventory_files_html_suffix(context: ReportContext<'_>) -> String {
    let mut suffix = context
        .inventory
        .files_scanned
        .map(|files| format!("; files scanned: <code>{files}</code>"))
        .unwrap_or_default();
    if let Some(completeness) = context.inventory.completeness {
        suffix.push_str(&format!(
            "; completeness: <code>{}</code>",
            html_escape(completeness)
        ));
    }
    suffix
}
