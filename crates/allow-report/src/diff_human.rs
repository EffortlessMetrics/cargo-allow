use crate::diff_finding_detail::structural_identity_summary;
use crate::diff_movement::{append_movement_summary_human, append_movement_summary_human_styled};
use crate::diff_policy_detail::policy_change_detail;
use crate::diff_posture::{
    diff_evidence_delta_summary, diff_net_posture, diff_posture_summary,
    diff_structural_delta_summary,
};
use crate::evidence_repair::evidence_repair_queues_from_counts;
use crate::ledger_posture::{FINDING_CHANGE_LABELS, coverage_movement_from_canonical_fields};
use crate::{DiffFindingChange, DiffLedgerMovementSummary, DiffPolicyChange, Style};
use allow_core::PresenceMovement;

const DIFF_HUMAN_CHANGE_LIMIT: usize = 120;

pub fn render_diff_posture_summary_human(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_posture_summary_human_styled(
        current_failures,
        finding_changes,
        policy_changes,
        Style::PLAIN,
    )
}

pub fn render_diff_posture_summary_human_styled(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    style: Style,
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_styled(
        current_failures,
        (0, 0, 0),
        finding_changes,
        policy_changes,
        empty_ledger_movement_summary(),
        style,
    )
}

pub fn render_diff_posture_summary_human_with_evidence_health_counts(
    current_failures: usize,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    ledger_movement: DiffLedgerMovementSummary,
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_styled(
        current_failures,
        (
            broken_evidence_links,
            missing_evidence,
            weak_evidence_references,
        ),
        finding_changes,
        policy_changes,
        ledger_movement,
        Style::PLAIN,
    )
}

pub fn render_diff_posture_summary_human_with_evidence_health_counts_styled(
    current_failures: usize,
    (broken_evidence_links, missing_evidence, weak_evidence_references): (usize, usize, usize),
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    ledger_movement: DiffLedgerMovementSummary,
    style: Style,
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_inner(
        current_failures,
        (
            broken_evidence_links,
            missing_evidence,
            weak_evidence_references,
        ),
        finding_changes,
        policy_changes,
        ledger_movement,
        style,
    )
}

pub fn render_diff_posture_summary_human_with_evidence_health(
    current_failures: usize,
    broken_evidence_links: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_styled(
        current_failures,
        (broken_evidence_links, 0, weak_evidence_references),
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
        Style::PLAIN,
    )
}

pub fn render_diff_posture_summary_human_with_evidence_health_styled(
    current_failures: usize,
    broken_evidence_links: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    style: Style,
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_styled(
        current_failures,
        (broken_evidence_links, 0, weak_evidence_references),
        finding_changes,
        policy_changes,
        empty_ledger_movement_summary(),
        style,
    )
}

fn empty_ledger_movement_summary() -> DiffLedgerMovementSummary {
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
    }
}

fn render_diff_posture_summary_human_with_evidence_health_counts_inner(
    current_failures: usize,
    (broken_evidence_links, missing_evidence, weak_evidence_references): (usize, usize, usize),
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
    ledger_movement: DiffLedgerMovementSummary,
    style: Style,
) -> String {
    let summary = diff_posture_summary(current_failures, finding_changes, policy_changes);
    let posture = diff_net_posture(summary);
    let mut out = String::new();
    out.push_str("\nDiff posture summary:\n");
    out.push_str(&format!(
        "  net_posture: {}\n",
        style_diff_status(style, posture.as_str())
    ));
    out.push_str(&format!(
        "  reviewer_action: {}\n",
        posture.reviewer_action()
    ));
    if style.is_enabled() {
        append_movement_summary_human_styled(&mut out, ledger_movement, style);
    } else {
        append_movement_summary_human(&mut out, ledger_movement);
    }
    out.push_str(&format!(
        "  current_check_failures: {}\n",
        summary.current_failures
    ));
    if broken_evidence_links > 0 {
        out.push_str(&format!(
            "  broken_evidence_links: {broken_evidence_links}\n"
        ));
    }
    if missing_evidence > 0 {
        out.push_str(&format!("  missing_evidence: {missing_evidence}\n"));
    }
    if weak_evidence_references > 0 {
        out.push_str(&format!(
            "  weak_evidence_references: {weak_evidence_references}\n"
        ));
    }
    let structural_delta = diff_structural_delta_summary(policy_changes);
    if structural_delta.scope_broadened > 0 {
        out.push_str(&format!(
            "  scope_broadened: {}\n",
            structural_delta.scope_broadened
        ));
    }
    if structural_delta.scope_changed > 0 {
        out.push_str(&format!(
            "  scope_changed: {}\n",
            structural_delta.scope_changed
        ));
    }
    if structural_delta.scope_narrowed > 0 {
        out.push_str(&format!(
            "  scope_narrowed: {}\n",
            structural_delta.scope_narrowed
        ));
    }
    if structural_delta.selector_changed > 0 {
        out.push_str(&format!(
            "  selector_changed: {}\n",
            structural_delta.selector_changed
        ));
    }
    if structural_delta.selector_precision_decreased > 0 {
        out.push_str(&format!(
            "  selector_precision_decreased: {}\n",
            structural_delta.selector_precision_decreased
        ));
    }
    if structural_delta.selector_precision_increased > 0 {
        out.push_str(&format!(
            "  selector_precision_increased: {}\n",
            structural_delta.selector_precision_increased
        ));
    }
    let evidence_delta = diff_evidence_delta_summary(policy_changes);
    if evidence_delta.evidence_added > 0 {
        out.push_str(&format!(
            "  evidence_added: {}\n",
            evidence_delta.evidence_added
        ));
    }
    if evidence_delta.weak_evidence_added > 0 {
        out.push_str(&format!(
            "  weak_evidence_added: {}\n",
            evidence_delta.weak_evidence_added
        ));
    }
    if evidence_delta.broken_evidence_added > 0 {
        out.push_str(&format!(
            "  broken_evidence_added: {}\n",
            evidence_delta.broken_evidence_added
        ));
    }
    if evidence_delta.evidence_removed > 0 {
        out.push_str(&format!(
            "  evidence_removed: {}\n",
            evidence_delta.evidence_removed
        ));
    }
    if evidence_delta.evidence_removal_failures > 0 {
        out.push_str(&format!(
            "  evidence_removal_failures: {}\n",
            evidence_delta.evidence_removal_failures
        ));
    }
    if evidence_delta.evidence_removal_review_items > 0 {
        out.push_str(&format!(
            "  evidence_removal_review_items: {}\n",
            evidence_delta.evidence_removal_review_items
        ));
    }
    if evidence_delta.evidence_removal_improvements > 0 {
        out.push_str(&format!(
            "  evidence_removal_improvements: {}\n",
            evidence_delta.evidence_removal_improvements
        ));
    }
    if evidence_delta.link_added > 0 {
        out.push_str(&format!("  link_added: {}\n", evidence_delta.link_added));
    }
    if evidence_delta.weak_link_added > 0 {
        out.push_str(&format!(
            "  weak_link_added: {}\n",
            evidence_delta.weak_link_added
        ));
    }
    if evidence_delta.broken_link_added > 0 {
        out.push_str(&format!(
            "  broken_link_added: {}\n",
            evidence_delta.broken_link_added
        ));
    }
    if evidence_delta.link_removed > 0 {
        out.push_str(&format!(
            "  link_removed: {}\n",
            evidence_delta.link_removed
        ));
    }
    if evidence_delta.link_removal_failures > 0 {
        out.push_str(&format!(
            "  link_removal_failures: {}\n",
            evidence_delta.link_removal_failures
        ));
    }
    if evidence_delta.link_removal_review_items > 0 {
        out.push_str(&format!(
            "  link_removal_review_items: {}\n",
            evidence_delta.link_removal_review_items
        ));
    }
    if evidence_delta.link_removal_improvements > 0 {
        out.push_str(&format!(
            "  link_removal_improvements: {}\n",
            evidence_delta.link_removal_improvements
        ));
    }
    let evidence_repair_queues = evidence_repair_queues_from_counts(
        broken_evidence_links,
        missing_evidence,
        weak_evidence_references,
        0,
    );
    if !evidence_repair_queues.is_empty() {
        out.push_str("  evidence_repair_queues:\n");
        for queue in evidence_repair_queues {
            out.push_str(&format!("    {}\n", queue.command));
        }
    }
    out.push_str(&format!(
        "  new_source_findings: {}\n",
        summary.new_findings
    ));
    out.push_str(&format!(
        "  removed_source_findings: {}\n",
        summary.removed_findings
    ));
    out.push_str(&format!("  policy_failures: {}\n", summary.policy_failures));
    out.push_str(&format!(
        "  policy_review_items: {}\n",
        summary.policy_review_items
    ));
    out.push_str(&format!(
        "  policy_improvements: {}\n",
        summary.policy_improvements
    ));
    out
}

pub fn render_diff_finding_changes_human(changes: &[DiffFindingChange<'_>]) -> String {
    render_diff_finding_changes_human_styled(changes, Style::PLAIN)
}

pub fn render_diff_finding_changes_human_styled(
    changes: &[DiffFindingChange<'_>],
    style: Style,
) -> String {
    let mut out = String::new();
    out.push_str("\nFinding posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    append_finding_changes_human_section(
        &mut out,
        "Finding attention",
        changes,
        PresenceMovement::Introduced.finding_change_label(),
        style,
    );
    append_finding_receipt_commands_human(&mut out, changes);
    append_finding_changes_human_section(
        &mut out,
        "Finding improvements",
        changes,
        PresenceMovement::Removed.finding_change_label(),
        style,
    );
    if changes
        .iter()
        .any(|change| !FINDING_CHANGE_LABELS.contains(&change.change))
    {
        out.push_str("  Other finding changes:\n");
        let other_count = changes
            .iter()
            .filter(|change| !FINDING_CHANGE_LABELS.contains(&change.change))
            .count();
        for change in changes
            .iter()
            .filter(|change| !FINDING_CHANGE_LABELS.contains(&change.change))
            .take(DIFF_HUMAN_CHANGE_LIMIT)
        {
            append_finding_change_human_row(&mut out, change, style);
        }
        if other_count > DIFF_HUMAN_CHANGE_LIMIT {
            out.push_str(&format!(
                "    ... {} more omitted\n",
                other_count - DIFF_HUMAN_CHANGE_LIMIT
            ));
        }
    }
    out
}

fn append_finding_changes_human_section(
    out: &mut String,
    heading: &str,
    changes: &[DiffFindingChange<'_>],
    change_kind: &str,
    style: Style,
) {
    if !changes.iter().any(|change| change.change == change_kind) {
        return;
    }
    out.push_str(&format!("  {heading}:\n"));
    let matching_count = changes
        .iter()
        .filter(|change| change.change == change_kind)
        .count();
    for change in changes
        .iter()
        .filter(|change| change.change == change_kind)
        .take(DIFF_HUMAN_CHANGE_LIMIT)
    {
        append_finding_change_human_row(out, change, style);
    }
    if matching_count > DIFF_HUMAN_CHANGE_LIMIT {
        out.push_str(&format!(
            "    ... {} more omitted\n",
            matching_count - DIFF_HUMAN_CHANGE_LIMIT
        ));
    }
}

/// Suggests `cargo-allow why` / `cargo-allow add --update` commands for each
/// introduced (unreceipted) finding so the reviewer can investigate and
/// receipt it without looking up the command syntax.
fn append_finding_receipt_commands_human(out: &mut String, changes: &[DiffFindingChange<'_>]) {
    let introduced: Vec<_> = changes
        .iter()
        .filter(|change| change.change == PresenceMovement::Introduced.finding_change_label())
        .take(DIFF_HUMAN_CHANGE_LIMIT)
        .collect();
    if introduced.is_empty() {
        return;
    }
    out.push_str("  Receipt commands:\n");
    for change in introduced {
        let line = change.line.unwrap_or(1);
        out.push_str(&format!(
            "    cargo-allow why --kind {} --path {} --line {line}\n",
            change.kind, change.path,
        ));
        out.push_str(&format!(
            "    cargo-allow add --kind {} --path {} --line {line} --owner <owner> --reason ... --evidence <ref> --update\n",
            change.kind, change.path,
        ));
    }
}

fn append_finding_change_human_row(out: &mut String, change: &DiffFindingChange<'_>, style: Style) {
    let source_package = change
        .source_package
        .map(|package| format!(" source_package={package}"))
        .unwrap_or_default();
    let identity = change
        .identity
        .map(|identity| format!(" identity={}", structural_identity_summary(identity)))
        .unwrap_or_default();
    let coverage_movement = row_coverage_movement(
        change.movement,
        change.posture_delta,
        change.changed_in_diff,
    );
    out.push_str(&format!(
        "    {} movement={} posture_delta={} coverage_movement={} changed_in_diff={} {}{} at {}{}{}\n",
        style_diff_status(style, change.change),
        style_diff_status(style, change.movement),
        style_diff_status(style, change.posture_delta),
        style_diff_status(style, coverage_movement),
        change.changed_in_diff,
        change.kind,
        change
            .family
            .map(|family| format!(".{family}"))
            .unwrap_or_default(),
        finding_location(change),
        source_package,
        identity
    ));
}

fn finding_location(change: &DiffFindingChange<'_>) -> String {
    match (change.line, change.column) {
        (Some(line), Some(column)) => format!("{}:{line}:{column}", change.path),
        (Some(line), None) => format!("{}:{line}", change.path),
        (None, Some(column)) => format!("{} column={column}", change.path),
        (None, None) => change.path.to_string(),
    }
}

pub fn render_diff_policy_changes_human(changes: &[DiffPolicyChange<'_>]) -> String {
    render_diff_policy_changes_human_styled(changes, Style::PLAIN)
}

pub fn render_diff_policy_changes_human_styled(
    changes: &[DiffPolicyChange<'_>],
    style: Style,
) -> String {
    let mut out = String::new();
    out.push_str("\nPolicy posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    append_policy_changes_human_section(&mut out, "Policy failures", changes, "fail", style);
    append_policy_changes_human_section(
        &mut out,
        "Policy review required",
        changes,
        "review",
        style,
    );
    append_policy_changes_human_section(
        &mut out,
        "Policy improvements",
        changes,
        "improvement",
        style,
    );
    let known_severities = ["fail", "review", "improvement"];
    if changes
        .iter()
        .any(|change| !known_severities.contains(&change.severity))
    {
        out.push_str("  Other policy changes:\n");
        for change in changes
            .iter()
            .filter(|change| !known_severities.contains(&change.severity))
        {
            append_policy_change_human_row(&mut out, change, style);
        }
    }
    out
}

fn append_policy_changes_human_section(
    out: &mut String,
    heading: &str,
    changes: &[DiffPolicyChange<'_>],
    severity: &str,
    style: Style,
) {
    if !changes.iter().any(|change| change.severity == severity) {
        return;
    }
    out.push_str(&format!("  {heading}:\n"));
    for change in changes {
        if change.severity == severity {
            append_policy_change_human_row(out, change, style);
        }
    }
}

fn append_policy_change_human_row(out: &mut String, change: &DiffPolicyChange<'_>, style: Style) {
    let coverage_movement = row_coverage_movement(
        change.movement,
        change.posture_delta,
        change.changed_in_diff,
    );
    out.push_str(&format!(
        "    {} movement={} posture_delta={} coverage_movement={} changed_in_diff={} {} {} {}: {}\n",
        style_diff_status(style, change.severity),
        style_diff_status(style, change.movement),
        style_diff_status(style, change.posture_delta),
        style_diff_status(style, coverage_movement),
        change.changed_in_diff,
        change.allow_id,
        change.kind,
        change
            .lane
            .map(|lane| format!("lane={lane}"))
            .unwrap_or_default(),
        change.message
    ));
    if let Some(detail) = policy_change_detail(change) {
        out.push_str(&format!("      detail: {detail}\n"));
    }
}

fn style_diff_status(style: Style, status: &str) -> String {
    match status {
        "new" | "introduced" | "fail" | "worsened" | "worse" => style.blocking(status),
        "removed" | "resolved" | "improvement" | "improved" => style.ok(status),
        _ => style.advisory(status),
    }
}

fn row_coverage_movement(
    movement: &str,
    posture_delta: &str,
    changed_in_diff: bool,
) -> &'static str {
    coverage_movement_from_canonical_fields(movement, posture_delta, changed_in_diff)
        .unwrap_or("retained")
}
