use crate::diff_posture::{diff_net_posture, diff_posture_summary};
use crate::text::markdown_cell;
use crate::{DiffFindingChange, DiffPolicyChange};

pub fn render_diff_pr_summary_markdown(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> String {
    let summary = diff_posture_summary(current_failures, finding_changes, policy_changes);
    let posture = diff_net_posture(summary);
    let mut out = String::new();
    out.push_str("## PR Summary\n\n");
    out.push_str(&format!("**Net posture:** `{}`\n\n", posture.as_str()));
    out.push_str("| Signal | Count |\n|---|---:|\n");
    out.push_str(&format!(
        "| Current check failures | {} |\n",
        summary.current_failures
    ));
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
    out.push_str(&format!(
        "\n**Reviewer action:** {}\n\n",
        posture.reviewer_action()
    ));
    append_finding_highlights(&mut out, finding_changes);
    append_policy_highlights(&mut out, policy_changes);
    out
}

fn append_finding_highlights(out: &mut String, finding_changes: &[DiffFindingChange<'_>]) {
    if finding_changes.iter().any(|change| change.change == "new") {
        out.push_str("### Finding Attention\n\n");
        out.push_str("| Change | Kind | Family | Path |\n|---|---|---|---|\n");
        for change in finding_changes
            .iter()
            .filter(|change| change.change == "new")
            .take(8)
        {
            append_finding_highlight_row(out, change);
        }
        out.push('\n');
    }

    if finding_changes
        .iter()
        .any(|change| change.change == "removed")
    {
        out.push_str("### Finding Improvements\n\n");
        out.push_str("| Change | Kind | Family | Path |\n|---|---|---|---|\n");
        for change in finding_changes
            .iter()
            .filter(|change| change.change == "removed")
            .take(8)
        {
            append_finding_highlight_row(out, change);
        }
        out.push('\n');
    }
}

fn append_finding_highlight_row(out: &mut String, change: &DiffFindingChange<'_>) {
    out.push_str(&format!(
        "| `{}` | `{}` | `{}` | `{}` |\n",
        markdown_cell(change.change),
        markdown_cell(change.kind),
        markdown_cell(change.family.unwrap_or("")),
        markdown_cell(change.path)
    ));
}

fn append_policy_highlights(out: &mut String, policy_changes: &[DiffPolicyChange<'_>]) {
    if policy_changes
        .iter()
        .any(|change| change.severity != "improvement")
    {
        out.push_str("### Policy Attention\n\n");
        out.push_str("| Severity | Allow ID | Kind | Message |\n|---|---|---|---|\n");
        for change in policy_changes
            .iter()
            .filter(|change| change.severity != "improvement")
            .take(8)
        {
            append_policy_highlight_row(out, change);
        }
        out.push('\n');
    }

    if policy_changes
        .iter()
        .any(|change| change.severity == "improvement")
    {
        out.push_str("### Policy Improvements\n\n");
        out.push_str("| Allow ID | Kind | Message |\n|---|---|---|\n");
        for change in policy_changes
            .iter()
            .filter(|change| change.severity == "improvement")
            .take(8)
        {
            out.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                markdown_cell(change.allow_id),
                markdown_cell(change.kind),
                markdown_cell(change.message)
            ));
        }
        out.push('\n');
    }
}

fn append_policy_highlight_row(out: &mut String, change: &DiffPolicyChange<'_>) {
    out.push_str(&format!(
        "| `{}` | `{}` | `{}` | {} |\n",
        markdown_cell(change.severity),
        markdown_cell(change.allow_id),
        markdown_cell(change.kind),
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
    out.push_str("| Change | Kind | Family | Path |\n|---|---|---|---|\n");
    for change in changes.iter().take(120) {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            markdown_cell(change.change),
            markdown_cell(change.kind),
            markdown_cell(change.family.unwrap_or("")),
            markdown_cell(change.path)
        ));
    }
    if changes.len() > 120 {
        out.push_str(&format!(
            "\n{} additional finding posture changes omitted.\n",
            changes.len() - 120
        ));
    }
    out
}

pub fn render_diff_policy_changes_markdown(changes: &[DiffPolicyChange<'_>]) -> String {
    let mut out = String::new();
    out.push_str("\n## Policy Posture Changes\n\n");
    if changes.is_empty() {
        out.push_str("No policy weakening detected.\n");
        return out;
    }
    out.push_str("| Severity | Allow ID | Kind | Message |\n|---|---|---|---|\n");
    for change in changes {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(change.severity),
            markdown_cell(change.allow_id),
            markdown_cell(change.kind),
            markdown_cell(change.message)
        ));
    }
    out
}
