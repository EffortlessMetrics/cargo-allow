use crate::diff_finding_detail::structural_identity_summary;
use crate::diff_movement::append_movement_summary_markdown;
use crate::diff_policy_detail::policy_change_detail;
use crate::diff_posture::{
    diff_evidence_delta_summary, diff_net_posture, diff_posture_summary,
    diff_structural_delta_summary,
};
use crate::evidence_repair::evidence_repair_queues_from_counts;
use crate::ledger_posture::{FINDING_CHANGE_LABELS, coverage_movement_from_canonical_fields};
use crate::text::markdown_cell;
use crate::{CLAIM_BOUNDARY_TEXT, DiffFindingChange, DiffLedgerMovementSummary, DiffPolicyChange};
use allow_core::PresenceMovement;

const PR_SUMMARY_HIGHLIGHT_LIMIT: usize = 8;
const DIFF_MARKDOWN_CHANGE_LIMIT: usize = 120;
const FINDING_ATTENTION_LABEL: &str = PresenceMovement::Introduced.finding_change_label();
const FINDING_IMPROVEMENT_LABEL: &str = PresenceMovement::Removed.finding_change_label();

pub fn render_diff_analysis_markdown(context: crate::DiffAnalysisContext<'_>) -> String {
    format!(
        "### Diff analysis\n\n- Result class: `{}`\n- Base revision: `{}`\n- Head revision: `{}`\n- Base inventory complete: `{}`\n- Base scanner complete: `{}`\n- Head inventory complete: `{}`\n- Head scanner complete: `{}`\n- Movement: introduced `{}`, retained `{}`, removed `{}`\n\n",
        context.result_class,
        context.base_revision.unwrap_or("<none>"),
        context.head_revision.unwrap_or("<none>"),
        context.base_inventory_complete,
        context.base_scanner_complete,
        context.head_inventory_complete,
        context.head_scanner_complete,
        context.introduced,
        context.retained,
        context.removed,
    )
}

pub fn render_diff_pr_summary_markdown(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_pr_summary_markdown_with_evidence_health(
        current_failures,
        0,
        0,
        finding_changes,
        policy_changes,
    )
}

pub fn render_diff_pr_summary_markdown_with_evidence_health_counts(
    current_failures: usize,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    ledger_movement: DiffLedgerMovementSummary,
) -> String {
    render_diff_pr_summary_markdown_with_evidence_health_counts_inner(
        current_failures,
        broken_evidence_links,
        missing_evidence,
        weak_evidence_references,
        finding_changes,
        policy_changes,
        ledger_movement,
    )
}

pub fn render_diff_pr_summary_markdown_with_evidence_health(
    current_failures: usize,
    broken_evidence_links: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_pr_summary_markdown_with_evidence_health_counts_inner(
        current_failures,
        broken_evidence_links,
        0,
        weak_evidence_references,
        finding_changes,
        policy_changes,
        DiffLedgerMovementSummary {
            movement: crate::DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: crate::DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
        },
    )
}

fn render_diff_pr_summary_markdown_with_evidence_health_counts_inner(
    current_failures: usize,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    ledger_movement: DiffLedgerMovementSummary,
) -> String {
    let summary = diff_posture_summary(current_failures, finding_changes, policy_changes);
    let posture = diff_net_posture(summary);
    let mut out = String::new();
    out.push_str("## PR Summary\n\n");
    out.push_str(&format!("**Net posture:** `{}`\n\n", posture.as_str()));
    out.push_str(&format!(
        "**Reviewer action:** {}\n\n",
        posture.reviewer_action()
    ));
    out.push_str("| Signal | Count |\n|---|---:|\n");
    out.push_str(&format!(
        "| Current check failures | {} |\n",
        summary.current_failures
    ));
    if broken_evidence_links > 0 {
        out.push_str(&format!(
            "| Broken evidence links | {broken_evidence_links} |\n"
        ));
    }
    if missing_evidence > 0 {
        out.push_str(&format!("| Missing evidence | {missing_evidence} |\n"));
    }
    if weak_evidence_references > 0 {
        out.push_str(&format!(
            "| Weak evidence/link references | {weak_evidence_references} |\n"
        ));
    }
    let structural_delta = diff_structural_delta_summary(policy_changes);
    if structural_delta.scope_broadened > 0 {
        out.push_str(&format!(
            "| Scope broadened | {} |\n",
            structural_delta.scope_broadened
        ));
    }
    if structural_delta.scope_changed > 0 {
        out.push_str(&format!(
            "| Scope changed | {} |\n",
            structural_delta.scope_changed
        ));
    }
    if structural_delta.scope_narrowed > 0 {
        out.push_str(&format!(
            "| Scope narrowed | {} |\n",
            structural_delta.scope_narrowed
        ));
    }
    if structural_delta.selector_changed > 0 {
        out.push_str(&format!(
            "| Selector changed | {} |\n",
            structural_delta.selector_changed
        ));
    }
    if structural_delta.selector_precision_decreased > 0 {
        out.push_str(&format!(
            "| Selector precision decreased | {} |\n",
            structural_delta.selector_precision_decreased
        ));
    }
    if structural_delta.selector_precision_increased > 0 {
        out.push_str(&format!(
            "| Selector precision increased | {} |\n",
            structural_delta.selector_precision_increased
        ));
    }
    let evidence_delta = diff_evidence_delta_summary(policy_changes);
    if evidence_delta.evidence_added > 0 {
        out.push_str(&format!(
            "| Evidence added | {} |\n",
            evidence_delta.evidence_added
        ));
    }
    if evidence_delta.weak_evidence_added > 0 {
        out.push_str(&format!(
            "| Weak evidence added | {} |\n",
            evidence_delta.weak_evidence_added
        ));
    }
    if evidence_delta.broken_evidence_added > 0 {
        out.push_str(&format!(
            "| Broken evidence added | {} |\n",
            evidence_delta.broken_evidence_added
        ));
    }
    if evidence_delta.evidence_removed > 0 {
        out.push_str(&format!(
            "| Evidence removed | {} |\n",
            evidence_delta.evidence_removed
        ));
    }
    if evidence_delta.evidence_removal_failures > 0 {
        out.push_str(&format!(
            "| Evidence removal failures | {} |\n",
            evidence_delta.evidence_removal_failures
        ));
    }
    if evidence_delta.evidence_removal_review_items > 0 {
        out.push_str(&format!(
            "| Evidence removal review items | {} |\n",
            evidence_delta.evidence_removal_review_items
        ));
    }
    if evidence_delta.evidence_removal_improvements > 0 {
        out.push_str(&format!(
            "| Evidence removal improvements | {} |\n",
            evidence_delta.evidence_removal_improvements
        ));
    }
    if evidence_delta.link_added > 0 {
        out.push_str(&format!(
            "| Links added | {} |\n",
            evidence_delta.link_added
        ));
    }
    if evidence_delta.weak_link_added > 0 {
        out.push_str(&format!(
            "| Weak links added | {} |\n",
            evidence_delta.weak_link_added
        ));
    }
    if evidence_delta.broken_link_added > 0 {
        out.push_str(&format!(
            "| Broken links added | {} |\n",
            evidence_delta.broken_link_added
        ));
    }
    if evidence_delta.link_removed > 0 {
        out.push_str(&format!(
            "| Links removed | {} |\n",
            evidence_delta.link_removed
        ));
    }
    if evidence_delta.link_removal_failures > 0 {
        out.push_str(&format!(
            "| Link removal failures | {} |\n",
            evidence_delta.link_removal_failures
        ));
    }
    if evidence_delta.link_removal_review_items > 0 {
        out.push_str(&format!(
            "| Link removal review items | {} |\n",
            evidence_delta.link_removal_review_items
        ));
    }
    if evidence_delta.link_removal_improvements > 0 {
        out.push_str(&format!(
            "| Link removal improvements | {} |\n",
            evidence_delta.link_removal_improvements
        ));
    }
    out.push_str(&format!(
        "| New source findings | {} |\n",
        summary.new_findings
    ));
    out.push_str(&format!(
        "| Removed source findings | {} |\n",
        summary.removed_findings
    ));
    out.push_str(&format!(
        "| Policy failures | {} |\n",
        summary.policy_failures
    ));
    out.push_str(&format!(
        "| Policy review items | {} |\n",
        summary.policy_review_items
    ));
    out.push_str(&format!(
        "| Policy improvements | {} |\n",
        summary.policy_improvements
    ));
    append_movement_summary_markdown(&mut out, ledger_movement);
    let evidence_repair_queues = evidence_repair_queues_from_counts(
        broken_evidence_links,
        missing_evidence,
        weak_evidence_references,
        0,
    );
    if !evidence_repair_queues.is_empty() {
        out.push_str("**Evidence repair queues:**\n");
        for queue in evidence_repair_queues {
            out.push_str(&format!("- `{}`\n", queue.command));
        }
        out.push('\n');
    }
    out.push_str("> ");
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push_str("\n\n");
    append_finding_highlights(&mut out, finding_changes);
    append_policy_highlights(&mut out, policy_changes);
    out
}

fn append_finding_highlights(out: &mut String, finding_changes: &[DiffFindingChange<'_>]) {
    let new_count = finding_changes
        .iter()
        .filter(|change| change.change == FINDING_ATTENTION_LABEL)
        .count();
    if new_count > 0 {
        out.push_str("### Finding Attention\n\n");
        let include_source_package =
            finding_changes_have_source_package(finding_changes, FINDING_ATTENTION_LABEL);
        let include_identity =
            finding_changes_have_identity(finding_changes, FINDING_ATTENTION_LABEL);
        append_finding_highlight_header(out, include_source_package, include_identity);
        for change in finding_changes
            .iter()
            .filter(|change| change.change == FINDING_ATTENTION_LABEL)
            .take(PR_SUMMARY_HIGHLIGHT_LIMIT)
        {
            append_finding_highlight_row(out, change, include_source_package, include_identity);
        }
        append_omitted_summary_note(out, new_count, "new finding change");
        out.push('\n');
    }

    let removed_count = finding_changes
        .iter()
        .filter(|change| change.change == FINDING_IMPROVEMENT_LABEL)
        .count();
    if removed_count > 0 {
        out.push_str("### Finding Improvements\n\n");
        let include_source_package =
            finding_changes_have_source_package(finding_changes, FINDING_IMPROVEMENT_LABEL);
        let include_identity =
            finding_changes_have_identity(finding_changes, FINDING_IMPROVEMENT_LABEL);
        append_finding_highlight_header(out, include_source_package, include_identity);
        for change in finding_changes
            .iter()
            .filter(|change| change.change == FINDING_IMPROVEMENT_LABEL)
            .take(PR_SUMMARY_HIGHLIGHT_LIMIT)
        {
            append_finding_highlight_row(out, change, include_source_package, include_identity);
        }
        append_omitted_summary_note(out, removed_count, "removed finding change");
        out.push('\n');
    }
}

fn append_finding_highlight_header(
    out: &mut String,
    include_source_package: bool,
    include_identity: bool,
) {
    out.push_str("| Change | Coverage Movement | Kind | Family | Path |");
    if include_source_package {
        out.push_str(" Source Package |");
    }
    if include_identity {
        out.push_str(" Identity |");
    }
    out.push_str("\n|---|---|---|---|---|");
    if include_source_package {
        out.push_str("---|");
    }
    if include_identity {
        out.push_str("---|");
    }
    out.push('\n');
}

fn append_finding_highlight_row(
    out: &mut String,
    change: &DiffFindingChange<'_>,
    include_source_package: bool,
    include_identity: bool,
) {
    let coverage_movement = row_coverage_movement(
        change.movement,
        change.posture_delta,
        change.changed_in_diff,
    );
    out.push_str(&format!(
        "| `{}` | `{}` | `{}` | `{}` | `{}` |",
        markdown_cell(change.change),
        markdown_cell(coverage_movement),
        markdown_cell(change.kind),
        markdown_cell(change.family.unwrap_or("")),
        markdown_cell(&finding_location(change))
    ));
    if include_source_package {
        out.push_str(&format!(
            " `{}` |",
            markdown_cell(change.source_package.unwrap_or(""))
        ));
    }
    if include_identity {
        out.push_str(&format!(
            " `{}` |",
            markdown_cell(&finding_identity_summary(change))
        ));
    }
    out.push('\n');
}

fn append_policy_highlights(out: &mut String, policy_changes: &[DiffPolicyChange<'_>]) {
    append_policy_severity_highlights(
        out,
        policy_changes,
        "fail",
        "### Policy Failures",
        "policy failure",
    );
    append_policy_severity_highlights(
        out,
        policy_changes,
        "review",
        "### Policy Review Required",
        "policy review item",
    );

    let improvement_count = policy_changes
        .iter()
        .filter(|change| change.severity == "improvement")
        .count();
    if improvement_count > 0 {
        out.push_str("### Policy Improvements\n\n");
        out.push_str("| Allow ID | Kind | Detail | Message |\n|---|---|---|---|\n");
        for change in policy_changes
            .iter()
            .filter(|change| change.severity == "improvement")
            .take(PR_SUMMARY_HIGHLIGHT_LIMIT)
        {
            let detail = policy_change_detail(change).unwrap_or_else(|| "none".to_string());
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} |\n",
                markdown_cell(change.allow_id),
                markdown_cell(change.kind),
                markdown_cell(&detail),
                markdown_cell(change.message)
            ));
        }
        append_omitted_summary_note(out, improvement_count, "policy improvement change");
        out.push('\n');
    }
}

fn append_policy_severity_highlights(
    out: &mut String,
    policy_changes: &[DiffPolicyChange<'_>],
    severity: &str,
    heading: &str,
    singular_label: &str,
) {
    let count = policy_changes
        .iter()
        .filter(|change| change.severity == severity)
        .count();
    if count == 0 {
        return;
    }

    out.push_str(heading);
    out.push_str("\n\n");
    out.push_str("| Severity | Allow ID | Kind | Coverage Movement | Detail | Message |\n|---|---|---|---|---|---|\n");
    for change in policy_changes
        .iter()
        .filter(|change| change.severity == severity)
        .take(PR_SUMMARY_HIGHLIGHT_LIMIT)
    {
        append_policy_highlight_row(out, change);
    }
    append_omitted_summary_note(out, count, singular_label);
    out.push('\n');
}

fn append_omitted_summary_note(out: &mut String, count: usize, singular_label: &str) {
    if count > PR_SUMMARY_HIGHLIGHT_LIMIT {
        let omitted = count - PR_SUMMARY_HIGHLIGHT_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "\n{omitted} additional {singular_label}{plural} omitted from this summary.\n"
        ));
    }
}

fn append_policy_highlight_row(out: &mut String, change: &DiffPolicyChange<'_>) {
    let detail = policy_change_detail(change).unwrap_or_else(|| "none".to_string());
    let coverage_movement = row_coverage_movement(
        change.movement,
        change.posture_delta,
        change.changed_in_diff,
    );
    out.push_str(&format!(
        "| `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
        markdown_cell(change.severity),
        markdown_cell(change.allow_id),
        markdown_cell(change.kind),
        markdown_cell(coverage_movement),
        markdown_cell(&detail),
        markdown_cell(change.message)
    ));
}

pub fn insert_markdown_pr_summary(text: &mut String, summary: &str) {
    let marker = "Findings scanned:";
    if let Some(index) = text.find(marker) {
        text.insert_str(index, summary);
    } else {
        text.push('\n');
        text.push_str(summary);
    }
}

pub fn render_diff_finding_changes_markdown(changes: &[DiffFindingChange<'_>]) -> String {
    let mut out = String::new();
    out.push_str("\n## Finding Posture Changes\n\n");
    if changes.is_empty() {
        out.push_str("No source finding posture changes detected.\n");
        return out;
    }
    append_finding_changes_markdown_section(
        &mut out,
        "Finding Attention",
        changes,
        FINDING_ATTENTION_LABEL,
    );
    append_finding_receipt_commands_markdown(&mut out, changes);
    append_finding_changes_markdown_section(
        &mut out,
        "Finding Improvements",
        changes,
        FINDING_IMPROVEMENT_LABEL,
    );
    if changes
        .iter()
        .any(|change| !FINDING_CHANGE_LABELS.contains(&change.change))
    {
        out.push_str("### Other Finding Changes\n\n");
        append_finding_changes_markdown_table(
            &mut out,
            changes
                .iter()
                .filter(|change| !FINDING_CHANGE_LABELS.contains(&change.change)),
        );
    }
    out
}

fn append_finding_changes_markdown_section<'a>(
    out: &mut String,
    heading: &str,
    changes: &'a [DiffFindingChange<'a>],
    change_kind: &str,
) {
    if !changes.iter().any(|change| change.change == change_kind) {
        return;
    }
    out.push_str(&format!("### {heading}\n\n"));
    append_finding_changes_markdown_table(
        out,
        changes.iter().filter(|change| change.change == change_kind),
    );
}

fn append_finding_receipt_commands_markdown(out: &mut String, changes: &[DiffFindingChange<'_>]) {
    let introduced: Vec<_> = changes
        .iter()
        .filter(|change| change.change == FINDING_ATTENTION_LABEL)
        .take(DIFF_MARKDOWN_CHANGE_LIMIT)
        .collect();
    if introduced.is_empty() {
        return;
    }
    out.push_str("### Receipt Commands\n\n");
    out.push_str("| Command |\n|---|\n");
    for change in introduced {
        let line = change.line.unwrap_or(1);
        out.push_str(&format!(
            "| `cargo-allow why --kind {} --path {} --line {line}` |\n",
            change.kind, change.path,
        ));
        out.push_str(&format!(
            "| `cargo-allow add --kind {} --path {} --line {line} --owner <owner> --reason ... --evidence <ref> --update` |\n",
            change.kind, change.path,
        ));
    }
    out.push('\n');
}

fn append_finding_changes_markdown_table<'a>(
    out: &mut String,
    changes: impl Iterator<Item = &'a DiffFindingChange<'a>>,
) {
    let changes = changes.collect::<Vec<_>>();
    let include_source_package = changes.iter().any(|change| change.source_package.is_some());
    let include_identity = changes.iter().any(|change| change.identity.is_some());
    append_finding_change_table_header(out, include_source_package, include_identity);
    for change in changes.iter().take(DIFF_MARKDOWN_CHANGE_LIMIT) {
        append_finding_change_markdown_row(out, change, include_source_package, include_identity);
    }
    if changes.len() > DIFF_MARKDOWN_CHANGE_LIMIT {
        out.push_str(&format!(
            "\n{} additional finding posture changes omitted.\n",
            changes.len() - DIFF_MARKDOWN_CHANGE_LIMIT
        ));
    }
    out.push('\n');
}

fn append_finding_change_table_header(
    out: &mut String,
    include_source_package: bool,
    include_identity: bool,
) {
    out.push_str("| Change | Coverage Movement | Kind | Family | Path |");
    if include_source_package {
        out.push_str(" Source Package |");
    }
    if include_identity {
        out.push_str(" Identity |");
    }
    out.push_str("\n|---|---|---|---|---|");
    if include_source_package {
        out.push_str("---|");
    }
    if include_identity {
        out.push_str("---|");
    }
    out.push('\n');
}

fn append_finding_change_markdown_row(
    out: &mut String,
    change: &DiffFindingChange<'_>,
    include_source_package: bool,
    include_identity: bool,
) {
    let coverage_movement = row_coverage_movement(
        change.movement,
        change.posture_delta,
        change.changed_in_diff,
    );
    out.push_str(&format!(
        "| `{}` | `{}` | `{}` | `{}` | `{}` |",
        markdown_cell(change.change),
        markdown_cell(coverage_movement),
        markdown_cell(change.kind),
        markdown_cell(change.family.unwrap_or("")),
        markdown_cell(&finding_location(change))
    ));
    if include_source_package {
        out.push_str(&format!(
            " `{}` |",
            markdown_cell(change.source_package.unwrap_or(""))
        ));
    }
    if include_identity {
        out.push_str(&format!(
            " `{}` |",
            markdown_cell(&finding_identity_summary(change))
        ));
    }
    out.push('\n');
}

fn finding_location(change: &DiffFindingChange<'_>) -> String {
    match (change.line, change.column) {
        (Some(line), Some(column)) => format!("{}:{line}:{column}", change.path),
        (Some(line), None) => format!("{}:{line}", change.path),
        (None, Some(column)) => format!("{} column={column}", change.path),
        (None, None) => change.path.to_string(),
    }
}

fn finding_identity_summary(change: &DiffFindingChange<'_>) -> String {
    change
        .identity
        .map(structural_identity_summary)
        .unwrap_or_default()
}

fn finding_changes_have_source_package(
    changes: &[DiffFindingChange<'_>],
    change_kind: &str,
) -> bool {
    changes
        .iter()
        .any(|change| change.change == change_kind && change.source_package.is_some())
}

fn finding_changes_have_identity(changes: &[DiffFindingChange<'_>], change_kind: &str) -> bool {
    changes
        .iter()
        .any(|change| change.change == change_kind && change.identity.is_some())
}

pub fn render_diff_policy_changes_markdown(changes: &[DiffPolicyChange<'_>]) -> String {
    let mut out = String::new();
    out.push_str("\n## Policy Posture Changes\n\n");
    if changes.is_empty() {
        out.push_str("No policy weakening detected.\n");
        return out;
    }
    append_policy_changes_markdown_section(&mut out, "Policy Failures", changes, "fail");
    append_policy_changes_markdown_section(&mut out, "Policy Review Required", changes, "review");
    append_policy_changes_markdown_section(&mut out, "Policy Improvements", changes, "improvement");
    let known_severities = ["fail", "review", "improvement"];
    if changes
        .iter()
        .any(|change| !known_severities.contains(&change.severity))
    {
        out.push_str("### Other Policy Changes\n\n");
        append_policy_changes_markdown_table(
            &mut out,
            changes
                .iter()
                .filter(|change| !known_severities.contains(&change.severity)),
        );
    }
    out
}

fn append_policy_changes_markdown_section<'a>(
    out: &mut String,
    heading: &str,
    changes: &'a [DiffPolicyChange<'a>],
    severity: &str,
) {
    if !changes.iter().any(|change| change.severity == severity) {
        return;
    }
    out.push_str(&format!("### {heading}\n\n"));
    append_policy_changes_markdown_table(
        out,
        changes.iter().filter(|change| change.severity == severity),
    );
}

fn append_policy_changes_markdown_table<'a>(
    out: &mut String,
    changes: impl Iterator<Item = &'a DiffPolicyChange<'a>>,
) {
    let changes = changes.collect::<Vec<_>>();
    out.push_str("| Severity | Allow ID | Kind | Coverage Movement | Detail | Message |\n|---|---|---|---|---|---|\n");
    for change in changes.iter().take(DIFF_MARKDOWN_CHANGE_LIMIT) {
        let detail = policy_change_detail(change).unwrap_or_else(|| "none".to_string());
        let coverage_movement = row_coverage_movement(
            change.movement,
            change.posture_delta,
            change.changed_in_diff,
        );
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
            markdown_cell(change.severity),
            markdown_cell(change.allow_id),
            markdown_cell(change.kind),
            markdown_cell(coverage_movement),
            markdown_cell(&detail),
            markdown_cell(change.message)
        ));
    }
    if changes.len() > DIFF_MARKDOWN_CHANGE_LIMIT {
        out.push_str(&format!(
            "\n{} additional policy posture changes omitted.\n",
            changes.len() - DIFF_MARKDOWN_CHANGE_LIMIT
        ));
    }
    out.push('\n');
}

fn row_coverage_movement(
    movement: &str,
    posture_delta: &str,
    changed_in_diff: bool,
) -> &'static str {
    coverage_movement_from_canonical_fields(movement, posture_delta, changed_in_diff)
        .unwrap_or("retained")
}
