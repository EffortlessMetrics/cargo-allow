use crate::diff_policy_detail::policy_change_detail;
use crate::diff_posture::{diff_net_posture, diff_posture_summary};
use crate::{DiffFindingChange, DiffPolicyChange};

pub fn render_diff_posture_summary_human(
    current_failures: usize,
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
    for change in changes.iter().take(120) {
        out.push_str(&format!(
            "  {} {}{} at {}\n",
            change.change,
            change.kind,
            change
                .family
                .map(|family| format!(".{family}"))
                .unwrap_or_default(),
            change.path
        ));
    }
    if changes.len() > 120 {
        out.push_str(&format!("  ... {} more omitted\n", changes.len() - 120));
    }
    out
}

pub fn render_diff_policy_changes_human(changes: &[DiffPolicyChange<'_>]) -> String {
    let mut out = String::new();
    out.push_str("\nPolicy posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    for change in changes {
        out.push_str(&format!(
            "  {} {} {}: {}\n",
            change.severity, change.allow_id, change.kind, change.message
        ));
        if let Some(detail) = policy_change_detail(change) {
            out.push_str(&format!("    detail: {detail}\n"));
        }
    }
    out
}
