use crate::diff_finding_detail::structural_identity_summary;
use crate::diff_policy_detail::policy_change_detail;
use crate::diff_posture::{diff_evidence_delta_summary, diff_net_posture, diff_posture_summary};
use crate::evidence_repair::evidence_repair_queues_from_counts;
use crate::{DiffFindingChange, DiffPolicyChange};

const DIFF_HUMAN_CHANGE_LIMIT: usize = 120;

pub fn render_diff_posture_summary_human(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_posture_summary_human_with_evidence_health(
        current_failures,
        0,
        0,
        finding_changes,
        policy_changes,
    )
}

pub fn render_diff_posture_summary_human_with_evidence_health_counts(
    current_failures: usize,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_inner(
        current_failures,
        broken_evidence_links,
        missing_evidence,
        weak_evidence_references,
        finding_changes,
        policy_changes,
    )
}

pub fn render_diff_posture_summary_human_with_evidence_health(
    current_failures: usize,
    broken_evidence_links: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    render_diff_posture_summary_human_with_evidence_health_counts_inner(
        current_failures,
        broken_evidence_links,
        0,
        weak_evidence_references,
        finding_changes,
        policy_changes,
    )
}

fn render_diff_posture_summary_human_with_evidence_health_counts_inner(
    current_failures: usize,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    let summary = diff_posture_summary(current_failures, finding_changes, policy_changes);
    let posture = diff_net_posture(summary);
    let mut out = String::new();
    out.push_str("\nDiff posture summary:\n");
    out.push_str(&format!("  net_posture: {}\n", posture.as_str()));
    out.push_str(&format!(
        "  reviewer_action: {}\n",
        posture.reviewer_action()
    ));
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
    let mut out = String::new();
    out.push_str("\nFinding posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    append_finding_changes_human_section(&mut out, "Finding attention", changes, "new");
    append_finding_changes_human_section(&mut out, "Finding improvements", changes, "removed");
    let known_changes = ["new", "removed"];
    if changes
        .iter()
        .any(|change| !known_changes.contains(&change.change))
    {
        out.push_str("  Other finding changes:\n");
        let other_count = changes
            .iter()
            .filter(|change| !known_changes.contains(&change.change))
            .count();
        for change in changes
            .iter()
            .filter(|change| !known_changes.contains(&change.change))
            .take(DIFF_HUMAN_CHANGE_LIMIT)
        {
            append_finding_change_human_row(&mut out, change);
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
        append_finding_change_human_row(out, change);
    }
    if matching_count > DIFF_HUMAN_CHANGE_LIMIT {
        out.push_str(&format!(
            "    ... {} more omitted\n",
            matching_count - DIFF_HUMAN_CHANGE_LIMIT
        ));
    }
}

fn append_finding_change_human_row(out: &mut String, change: &DiffFindingChange<'_>) {
    let source_package = change
        .source_package
        .map(|package| format!(" source_package={package}"))
        .unwrap_or_default();
    let identity = change
        .identity
        .map(|identity| format!(" identity={}", structural_identity_summary(identity)))
        .unwrap_or_default();
    out.push_str(&format!(
        "    {} {}{} at {}{}{}\n",
        change.change,
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
    let mut out = String::new();
    out.push_str("\nPolicy posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    append_policy_changes_human_section(&mut out, "Policy failures", changes, "fail");
    append_policy_changes_human_section(&mut out, "Policy review required", changes, "review");
    append_policy_changes_human_section(&mut out, "Policy improvements", changes, "improvement");
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
            append_policy_change_human_row(&mut out, change);
        }
    }
    out
}

fn append_policy_changes_human_section(
    out: &mut String,
    heading: &str,
    changes: &[DiffPolicyChange<'_>],
    severity: &str,
) {
    if !changes.iter().any(|change| change.severity == severity) {
        return;
    }
    out.push_str(&format!("  {heading}:\n"));
    for change in changes {
        if change.severity == severity {
            append_policy_change_human_row(out, change);
        }
    }
}

fn append_policy_change_human_row(out: &mut String, change: &DiffPolicyChange<'_>) {
    out.push_str(&format!(
        "    {} {} {}: {}\n",
        change.severity, change.allow_id, change.kind, change.message
    ));
    if let Some(detail) = policy_change_detail(change) {
        out.push_str(&format!("      detail: {detail}\n"));
    }
}
