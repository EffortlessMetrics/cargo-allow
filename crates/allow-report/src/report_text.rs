use crate::audit_remediation::audit_remediation_items;
use crate::evidence_repair::{
    BROKEN_EVIDENCE_LINK_COMMAND, WEAK_EVIDENCE_REFERENCE_COMMAND, evidence_repair_queues,
};
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
    append_empty_git_tracked_warning_human(context, &mut out);
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
    if context.rust_files_skipped > 0 {
        out.push_str(&format!(
            "  warning: {} rust source file{} skipped (oversized, binary, or unreadable); \
             check --mode no-new fails closed when files are skipped\n",
            context.rust_files_skipped,
            if context.rust_files_skipped == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    if command == "audit" {
        render_source_inventory_human(findings, outcomes, &mut out);
        render_audit_summary_human(&summary, outcomes, context, &mut out);
    }
    render_non_rust_human(findings, outcomes, &mut out);
    if command != "audit" {
        let signals = ReviewSignals::from_summary(&summary, context);
        append_remediation_roadmap_human(&summary, signals, &mut out);
        append_evidence_repair_queues_human(&summary, signals, &mut out);
    }
    out.push('\n');
    // `--quiet` drops advisory outcomes but never the blocking ones: those are
    // the reason the run failed, and hiding them would leave an operator
    // staring at a bare non-zero exit (#2785).
    let quiet = crate::contracts::is_quiet();
    let non_matched = outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .filter(|o| !quiet || blocks_check(o.status))
        .collect::<Vec<_>>();
    for outcome in non_matched.iter().take(HUMAN_NON_MATCHED_OUTCOME_LIMIT) {
        out.push_str(&format!(
            "{}: {}\n",
            outcome.status.as_str(),
            outcome.message
        ));
    }
    append_human_omitted_outcome_note(&mut out, non_matched.len());
    if !crate::contracts::is_quiet() {
        out.push('\n');
        out.push_str(CLAIM_BOUNDARY_TEXT);
        out.push('\n');
    }
    if failed {
        match check_failure_reason(&summary) {
            Some(reason) => out.push_str(&format!("Result: failed ({reason})\n")),
            None => out.push_str("Result: failed\n"),
        }
    } else {
        out.push_str(&format!(
            "Result: {}\n",
            crate::contracts::passed_result_label(context.enforcement)
        ));
    }
    out
}

/// Statuses that can fail a check, as opposed to advisory ones like
/// `location_drift`, `stale`, and `review_due`.
///
/// Single source of truth for two callers that must agree: the failure reason
/// appended to `Result: failed (...)`, and the `--quiet` outcome filter. If
/// they drifted, `--quiet` could suppress the one line explaining a non-zero
/// exit.
const BLOCKING_STATUSES: [MatchStatus; 7] = [
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::MissingRequiredField,
    MatchStatus::EvidenceMissing,
    MatchStatus::BaselineDebt,
];

fn blocks_check(status: MatchStatus) -> bool {
    BLOCKING_STATUSES.contains(&status)
}

/// Build a human-readable reason for why a check failed, based on blocking
/// status counts from the summary. Returns None if no blocking statuses are
/// present (the failure may come from evidence links, federation divergence,
/// inventory parse errors, or other out-of-band conditions).
fn check_failure_reason(summary: &Summary) -> Option<String> {
    let mut parts: Vec<(String, usize)> = Vec::new();
    for status in BLOCKING_STATUSES {
        let count = summary.count(status);
        if count > 0 {
            parts.push((status.as_str().replace('_', " "), count));
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|(label, count)| format!("{count} {label}"))
            .collect::<Vec<_>>()
            .join(", "),
    )
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
        "review_due",
        summary.count(MatchStatus::ReviewDue)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "stale",
        summary.count(MatchStatus::Stale)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "ambiguous",
        summary.count(MatchStatus::Ambiguous)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "invalid_selector",
        summary.count(MatchStatus::InvalidSelector)
    ));
    out.push_str(&format!(
        "  {:24} {}\n",
        "missing_required_field",
        summary.count(MatchStatus::MissingRequiredField)
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
    out.push_str(&audit_recommended_next_step(
        summary,
        signals,
        queue.is_empty(),
    ));
    append_audit_remediation_roadmap_human(summary, signals, out);
    append_evidence_repair_queues_human(summary, signals, out);
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
        if failed {
            match check_failure_reason(&summary) {
                Some(reason) => format!("failed ({reason})"),
                None => "failed".to_string(),
            }
        } else {
            crate::contracts::passed_result_label(context.enforcement)
        }
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
    append_empty_git_tracked_warning_markdown(context, &mut out);
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
    } else if command == "check" {
        render_advisory_summary_markdown(&summary, context, &mut out);
    }
    render_non_rust_markdown(findings, outcomes, &mut out);
    if command != "audit" {
        let signals = ReviewSignals::from_summary(&summary, context);
        append_remediation_roadmap_markdown(&summary, signals, &mut out);
        append_evidence_repair_queues_markdown(&summary, signals, &mut out);
    }
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

fn render_advisory_summary_markdown(
    summary: &Summary,
    context: ReportContext<'_>,
    out: &mut String,
) {
    let signals = ReviewSignals::from_summary(summary, context);
    out.push_str("\n## Advisory counts\n\n");
    out.push_str("| Signal | Count |\n|---|---:|\n");
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
        "| Review due | {} |\n",
        summary.count(MatchStatus::ReviewDue)
    ));
    out.push_str(&format!(
        "| Stale | {} |\n",
        summary.count(MatchStatus::Stale)
    ));
    out.push_str(&format!(
        "| Ambiguous | {} |\n",
        summary.count(MatchStatus::Ambiguous)
    ));
    out.push_str(&format!(
        "| Invalid selectors | {} |\n",
        summary.count(MatchStatus::InvalidSelector)
    ));
    out.push_str(&format!(
        "| Missing required fields | {} |\n",
        summary.count(MatchStatus::MissingRequiredField)
    ));
    out.push_str(&format!(
        "| Evidence gaps | {} |\n",
        summary.count(MatchStatus::EvidenceMissing)
    ));
    out.push_str(&format!(
        "| Policy missing evidence | {} |\n",
        signals.policy_missing_evidence
    ));
    if signals.broken_evidence_links > 0 {
        out.push_str(&format!(
            "| Broken evidence links | {} |\n",
            signals.broken_evidence_links
        ));
    }
    if signals.weak_evidence_references > 0 {
        out.push_str(&format!(
            "| Weak evidence references | {} |\n",
            signals.weak_evidence_references
        ));
    }
    out.push_str(&format!("| Baseline debt | {} |\n", signals.baseline_debt));
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
        "| Review due | {} |\n",
        summary.count(MatchStatus::ReviewDue)
    ));
    out.push_str(&format!(
        "| Stale | {} |\n",
        summary.count(MatchStatus::Stale)
    ));
    out.push_str(&format!(
        "| Ambiguous | {} |\n",
        summary.count(MatchStatus::Ambiguous)
    ));
    out.push_str(&format!(
        "| Invalid selectors | {} |\n",
        summary.count(MatchStatus::InvalidSelector)
    ));
    out.push_str(&format!(
        "| Missing required fields | {} |\n",
        summary.count(MatchStatus::MissingRequiredField)
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
    out.push_str(&audit_recommended_next_step(
        summary,
        signals,
        queue.is_empty(),
    ));
    append_audit_remediation_roadmap_markdown(summary, signals, out);
    append_evidence_repair_queues_markdown(summary, signals, out);

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

fn append_evidence_repair_queues_human(
    summary: &Summary,
    signals: ReviewSignals,
    out: &mut String,
) {
    let commands = evidence_repair_commands(summary, signals);
    if commands.is_empty() {
        return;
    }
    out.push_str("\nEvidence repair queues:\n");
    for command in commands {
        out.push_str(&format!("  {command}\n"));
    }
}

fn append_evidence_repair_queues_markdown(
    summary: &Summary,
    signals: ReviewSignals,
    out: &mut String,
) {
    let commands = evidence_repair_commands(summary, signals);
    if commands.is_empty() {
        return;
    }
    out.push_str("\n### Evidence Repair Queues\n\n");
    for command in commands {
        out.push_str(&format!("- `{command}`\n"));
    }
}

fn evidence_repair_commands(summary: &Summary, signals: ReviewSignals) -> Vec<&'static str> {
    evidence_repair_queues(summary, signals)
        .into_iter()
        .map(|queue| queue.command)
        .collect()
}

fn append_audit_remediation_roadmap_human(
    summary: &Summary,
    signals: ReviewSignals,
    out: &mut String,
) {
    let items = audit_remediation_items(summary, signals);
    if items.is_empty() {
        return;
    }
    out.push_str("\nAudit remediation roadmap:\n");
    for item in items {
        out.push_str(&format!("  {}: {}\n", item.label, item.command));
    }
}

fn append_audit_remediation_roadmap_markdown(
    summary: &Summary,
    signals: ReviewSignals,
    out: &mut String,
) {
    let items = audit_remediation_items(summary, signals);
    if items.is_empty() {
        return;
    }
    out.push_str("\n## Audit Remediation Roadmap\n\n");
    out.push_str("| Signal | Command |\n|---|---|\n");
    for item in items {
        out.push_str(&format!("| {} | `{}` |\n", item.label, item.command));
    }
}

/// Status-neutral remediation roadmap for non-audit commands (e.g. `check`).
/// Surfaces the same copy-paste commands as the audit roadmap so a failed
/// `check --mode no-new` tells the operator what to run next.
fn append_remediation_roadmap_human(summary: &Summary, signals: ReviewSignals, out: &mut String) {
    let items = audit_remediation_items(summary, signals);
    if items.is_empty() {
        return;
    }
    out.push_str("\nRemediation roadmap:\n");
    for item in items {
        out.push_str(&format!("  {}: {}\n", item.label, item.command));
    }
    if summary.count(MatchStatus::New) > 0 {
        out.push_str(
            "  To diagnose and receipt a new finding: cargo-allow why --kind <kind> --path <path> --line <line>\n",
        );
    }
}

fn append_remediation_roadmap_markdown(
    summary: &Summary,
    signals: ReviewSignals,
    out: &mut String,
) {
    let items = audit_remediation_items(summary, signals);
    if items.is_empty() {
        return;
    }
    out.push_str("\n## Remediation Roadmap\n\n");
    out.push_str("| Signal | Command |\n|---|---|\n");
    for item in items {
        out.push_str(&format!("| {} | `{}` |\n", item.label, item.command));
    }
    if summary.count(MatchStatus::New) > 0 {
        out.push_str("\nTo diagnose and receipt a new finding: `cargo-allow why --kind <kind> --path <path> --line <line>`\n");
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
) -> String {
    if signals.review_items == 0 {
        "\nRecommended next step: keep `cargo-allow check --mode no-new` in CI.\n".to_string()
    } else if queue_empty && signals.broken_evidence_links > 0 {
        format!(
            "\nRecommended next step: run `{BROKEN_EVIDENCE_LINK_COMMAND}` to repair broken local evidence/link references.\n"
        )
    } else if queue_empty
        && signals.policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing)
    {
        "\nRecommended next step: run `cargo-allow worklist --format json` to route retained entries with no evidence references; add `--missing-evidence` to focus that queue.\n".to_string()
    } else if queue_empty && signals.weak_evidence_references > 0 {
        format!(
            "\nRecommended next step: run `{WEAK_EVIDENCE_REFERENCE_COMMAND}` to replace unstructured or unknown-prefix evidence/link references.\n"
        )
    } else if queue_empty && signals.baseline_debt > 0 {
        "\nRecommended next step: run `cargo-allow worklist --format json` to review generated baseline debt.\n".to_string()
    } else {
        "\nRecommended next step: review the queue below before tightening policy.\n".to_string()
    }
}

fn inventory_files_suffix(context: ReportContext<'_>) -> String {
    context.inventory.files_scanned_suffix()
}

fn inventory_files_markdown_suffix(context: ReportContext<'_>) -> String {
    let mut suffix = context
        .inventory
        .files_scanned
        .map(|files| format!("; files scanned: `{files}`"))
        .unwrap_or_default();
    if let Some(completeness) = context.inventory.completeness {
        suffix.push_str(&format!("; completeness: `{completeness}`"));
    }
    suffix
}

fn append_empty_git_tracked_warning_human(context: ReportContext<'_>, out: &mut String) {
    if context.inventory.empty_git_tracked {
        out.push_str(
            "Inventory warning: git reported no tracked files; newly initialized repos need `git add` or `--include-untracked` before cargo-allow scans source files.\n",
        );
    }
}

fn append_empty_git_tracked_warning_markdown(context: ReportContext<'_>, out: &mut String) {
    if context.inventory.empty_git_tracked {
        out.push_str(
            "Inventory warning: Git reported no tracked files; newly initialized repos need `git add` or `--include-untracked` before cargo-allow scans source files.\n\n",
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(status: MatchStatus, message: &str) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: None,
            candidate_ids: Vec::new(),
            finding_index: None,
            message: message.to_string(),
            score: 0,
        }
    }

    fn audit_queue_outcomes(count: usize) -> Vec<MatchOutcome> {
        (0..count)
            .map(|index| outcome(MatchStatus::New, &format!("new source exception {index}")))
            .collect()
    }

    fn review_outcomes() -> Vec<MatchOutcome> {
        vec![
            outcome(MatchStatus::New, "new source exception"),
            outcome(MatchStatus::Expired, "expired policy entry"),
            outcome(MatchStatus::ReviewDue, "policy entry review is due"),
            outcome(MatchStatus::Stale, "stale policy entry"),
            outcome(MatchStatus::Ambiguous, "ambiguous policy selector"),
            outcome(MatchStatus::InvalidSelector, "invalid selector"),
            outcome(MatchStatus::MissingRequiredField, "missing owner field"),
            outcome(MatchStatus::EvidenceMissing, "missing evidence reference"),
        ]
    }

    fn evidence_context() -> ReportContext<'static> {
        let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(3));
        context.policy_missing_evidence_entries = Some(4);
        context.broken_evidence_links = Some(2);
        context.weak_evidence_references = Some(1);
        context
    }

    fn review_signals(
        baseline_debt: usize,
        policy_missing_evidence: usize,
        broken_evidence_links: usize,
        weak_evidence_references: usize,
        occurrence_headroom: usize,
        mirror_divergence: usize,
        review_items: usize,
    ) -> ReviewSignals {
        ReviewSignals {
            baseline_debt,
            policy_missing_evidence,
            broken_evidence_links,
            weak_evidence_references,
            occurrence_headroom,
            mirror_divergence,
            review_items,
        }
    }

    #[test]
    fn audit_summary_human_lists_review_counts_and_repair_routes() {
        let outcomes = review_outcomes();
        let summary = Summary::from_outcomes(&outcomes);
        let mut out = String::new();

        render_audit_summary_human(&summary, &outcomes, evidence_context(), &mut out);

        assert!(out.contains("Audit summary:"));
        assert!(out.contains("match_outcomes"));
        assert!(out.contains("review_items"));
        assert!(out.contains("new_unreceipted"));
        assert!(out.contains("expired"));
        assert!(out.contains("review_due"));
        assert!(out.contains("stale"));
        assert!(out.contains("ambiguous"));
        assert!(out.contains("invalid_selector"));
        assert!(out.contains("missing_required_field"));
        assert!(out.contains("evidence_gaps"));
        assert!(out.contains("policy_missing_evidence"));
        assert!(out.contains("broken_evidence_links"));
        assert!(out.contains("weak_evidence_references"));
        assert!(out.contains("baseline_debt"));
        assert!(out.contains("Recommended next step: review the queue below"));
        assert!(out.contains("Audit remediation roadmap:"));
        assert!(out.contains("cargo-allow worklist --status new --format json"));
        assert!(out.contains("cargo-allow prune --stale --dry-run --format json"));
        assert!(out.contains("cargo-allow worklist --broken-evidence --format json"));
        assert!(out.contains("Evidence repair queues:"));
        assert!(out.contains("cargo-allow worklist --missing-evidence --format json"));
        assert!(out.contains("cargo-allow worklist --weak-evidence --format json"));
        assert!(out.contains("Audit review queue:"));
        assert!(out.contains("new: new source exception"));
    }

    #[test]
    fn audit_summary_markdown_lists_review_counts_and_repair_routes() {
        let outcomes = review_outcomes();
        let summary = Summary::from_outcomes(&outcomes);
        let mut out = String::new();

        render_audit_summary_markdown(&summary, &outcomes, evidence_context(), &mut out);

        assert!(out.contains("## Audit Summary"));
        assert!(out.contains("| Match outcomes | 8 |"));
        assert!(out.contains("| Review items | 17 |"));
        assert!(out.contains("| New unreceipted | 1 |"));
        assert!(out.contains("| Expired | 1 |"));
        assert!(out.contains("| Review due | 1 |"));
        assert!(out.contains("| Stale | 1 |"));
        assert!(out.contains("| Ambiguous | 1 |"));
        assert!(out.contains("| Invalid selectors | 1 |"));
        assert!(out.contains("| Missing required fields | 1 |"));
        assert!(out.contains("| Evidence gaps | 1 |"));
        assert!(out.contains("| Policy missing evidence | 4 |"));
        assert!(out.contains("| Broken evidence links | 2 |"));
        assert!(out.contains("| Weak evidence/link references | 1 |"));
        assert!(out.contains("| Baseline debt | 3 |"));
        assert!(out.contains("## Audit Remediation Roadmap"));
        assert!(
            out.contains("| new unreceipted | `cargo-allow worklist --status new --format json` |")
        );
        assert!(out.contains(
            "| broken evidence links | `cargo-allow worklist --broken-evidence --format json` |"
        ));
        assert!(out.contains("### Evidence Repair Queues"));
        assert!(out.contains("- `cargo-allow worklist --missing-evidence --format json`"));
        assert!(out.contains("## Audit Review Queue"));
        assert!(out.contains("- `new`: new source exception"));
    }

    #[test]
    fn audit_recommended_next_step_routes_empty_queue_evidence_signals() {
        let summary = Summary::from_outcomes(&[]);

        assert_eq!(
            audit_recommended_next_step(&summary, review_signals(0, 0, 0, 0, 0, 0, 0), true),
            "\nRecommended next step: keep `cargo-allow check --mode no-new` in CI.\n"
        );
        assert_eq!(
            audit_recommended_next_step(&summary, review_signals(0, 0, 1, 0, 0, 0, 1), true),
            "\nRecommended next step: run `cargo-allow worklist --broken-evidence --format json` to repair broken local evidence/link references.\n"
        );
        assert_eq!(
            audit_recommended_next_step(&summary, review_signals(0, 1, 0, 0, 0, 0, 1), true),
            "\nRecommended next step: run `cargo-allow worklist --format json` to route retained entries with no evidence references; add `--missing-evidence` to focus that queue.\n"
        );
        assert_eq!(
            audit_recommended_next_step(&summary, review_signals(0, 0, 0, 1, 0, 0, 1), true),
            "\nRecommended next step: run `cargo-allow worklist --weak-evidence --format json` to replace unstructured or unknown-prefix evidence/link references.\n"
        );
        assert_eq!(
            audit_recommended_next_step(&summary, review_signals(1, 0, 0, 0, 0, 0, 1), true),
            "\nRecommended next step: run `cargo-allow worklist --format json` to review generated baseline debt.\n"
        );
        assert_eq!(
            audit_recommended_next_step(&summary, review_signals(0, 0, 0, 0, 0, 0, 1), false),
            "\nRecommended next step: review the queue below before tightening policy.\n"
        );
    }

    #[test]
    fn omitted_review_queue_notes_report_extra_items() {
        let mut human = String::new();
        append_human_omitted_review_queue_note(&mut human, AUDIT_REVIEW_QUEUE_LIMIT + 2);
        assert!(human.contains("2 additional audit review items omitted from this queue"));

        let mut markdown = String::new();
        append_markdown_omitted_review_queue_note(&mut markdown, AUDIT_REVIEW_QUEUE_LIMIT + 1);
        assert!(markdown.contains("1 additional audit review item omitted from this queue."));
    }

    #[test]
    fn omitted_non_matched_notes_report_extra_items() {
        let mut human = String::new();
        append_human_omitted_outcome_note(&mut human, HUMAN_NON_MATCHED_OUTCOME_LIMIT + 1);
        assert!(human.contains("1 additional non-matched outcome omitted from this listing"));

        let mut markdown = String::new();
        append_markdown_omitted_outcome_note(&mut markdown, MARKDOWN_NON_MATCHED_OUTCOME_LIMIT + 2);
        assert!(markdown.contains("2 additional non-matched outcomes omitted from this listing."));
    }

    #[test]
    fn audit_summary_omits_review_queue_only_when_queue_is_empty() {
        let outcomes = audit_queue_outcomes(AUDIT_REVIEW_QUEUE_LIMIT + 1);
        let summary = Summary::from_outcomes(&outcomes);
        let mut human = String::new();
        let mut markdown = String::new();

        render_audit_summary_human(&summary, &outcomes, ReportContext::default(), &mut human);
        render_audit_summary_markdown(&summary, &outcomes, ReportContext::default(), &mut markdown);

        assert!(human.contains("Audit review queue:"));
        assert!(human.contains("1 additional audit review item omitted from this queue"));
        assert!(markdown.contains("## Audit Review Queue"));
        assert!(markdown.contains("1 additional audit review item omitted from this queue."));

        let empty_summary = Summary::from_outcomes(&[]);
        let mut empty = String::new();
        render_audit_summary_human(&empty_summary, &[], ReportContext::default(), &mut empty);
        assert!(!empty.contains("Audit review queue:"));
    }

    #[test]
    fn policy_context_notes_only_report_policy_excess() {
        let outcomes = vec![
            outcome(
                MatchStatus::EvidenceMissing,
                "matched entry has no evidence",
            ),
            outcome(MatchStatus::BaselineDebt, "generated baseline debt"),
        ];
        let summary = Summary::from_outcomes(&outcomes);
        let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(3));
        context.policy_missing_evidence_entries = Some(4);

        assert_eq!(policy_baseline_debt_note(&summary, context), Some(3));
        assert_eq!(policy_missing_evidence_note(&summary, context), Some(4));

        let mut matching_context = ReportContext::source_syntax("git_tracked", None, None, Some(1));
        matching_context.policy_missing_evidence_entries = Some(1);

        assert_eq!(policy_baseline_debt_note(&summary, matching_context), None);
        assert_eq!(
            policy_missing_evidence_note(&summary, matching_context),
            None
        );
    }

    #[test]
    fn check_failure_reason_lists_blocking_status_counts() {
        let outcomes = review_outcomes();
        let summary = Summary::from_outcomes(&outcomes);
        let reason = check_failure_reason(&summary);

        assert!(reason.is_some(), "should have a reason");
        let reason = reason.unwrap_or_default();

        assert!(
            reason.contains("1 new"),
            "reason should include new count: {reason}"
        );
        assert!(
            reason.contains("1 expired"),
            "reason should include expired count: {reason}"
        );
        assert!(
            reason.contains("1 ambiguous"),
            "reason should include ambiguous count: {reason}"
        );
        assert!(
            reason.contains("1 evidence missing"),
            "reason should include evidence missing: {reason}"
        );
    }

    #[test]
    fn check_failure_reason_returns_none_when_all_matched() {
        let outcomes = vec![
            outcome(MatchStatus::Matched, "matched"),
            outcome(MatchStatus::Matched, "matched"),
        ];
        let summary = Summary::from_outcomes(&outcomes);

        assert!(check_failure_reason(&summary).is_none());
    }

    #[test]
    fn render_human_includes_failure_reason_on_failed_check() {
        let outcomes = vec![
            outcome(MatchStatus::New, "new source exception"),
            outcome(MatchStatus::Expired, "expired policy entry"),
        ];
        let text = render_human("check", &[], &outcomes, true);

        assert!(
            text.contains("Result: failed (1 new"),
            "failed result should include blocking reason: `{text}`"
        );
        assert!(
            text.contains("1 expired"),
            "failed result should include expired count: `{text}`"
        );
    }

    #[test]
    fn render_human_shows_plain_failed_when_no_blocking_outcomes() {
        // Failure can come from evidence links or federation, not just outcomes.
        let text = render_human("check", &[], &[], true);

        assert!(
            text.contains("Result: failed\n"),
            "failed with no blocking outcomes should be plain: `{text}`"
        );
    }
}
