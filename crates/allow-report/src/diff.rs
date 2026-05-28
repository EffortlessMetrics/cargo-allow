use allow_core::json_escape;

use crate::json::option_json;
use crate::text::markdown_cell;
use crate::{DiffFindingChange, DiffPolicyChange, DiffPostureSummary, DiffReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffNetPosture {
    Worse,
    ReviewRequired,
    Improved,
    Unchanged,
}

impl DiffNetPosture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Worse => "worse",
            Self::ReviewRequired => "review-required",
            Self::Improved => "improved",
            Self::Unchanged => "unchanged",
        }
    }

    pub fn reviewer_action(self) -> &'static str {
        match self {
            Self::Worse => {
                "block until failing source exception changes are fixed, narrowed, or receipted."
            }
            Self::ReviewRequired => "review the source exception posture change before merging.",
            Self::Improved => "verify the cleanup was intentional and keep the narrower posture.",
            Self::Unchanged => "no source exception posture change detected.",
        }
    }
}

pub fn diff_posture_summary(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> DiffPostureSummary {
    DiffPostureSummary {
        current_failures,
        new_findings: finding_changes
            .iter()
            .filter(|change| change.change == "new")
            .count(),
        removed_findings: finding_changes
            .iter()
            .filter(|change| change.change == "removed")
            .count(),
        policy_failures: policy_changes
            .iter()
            .filter(|change| change.severity == "fail")
            .count(),
        policy_review_items: policy_changes
            .iter()
            .filter(|change| change.severity == "review")
            .count(),
        policy_improvements: policy_changes
            .iter()
            .filter(|change| change.severity == "improvement")
            .count(),
    }
}

pub fn diff_net_posture(summary: DiffPostureSummary) -> DiffNetPosture {
    if summary.current_failures > 0 || summary.policy_failures > 0 {
        return DiffNetPosture::Worse;
    }
    if summary.new_findings > 0 || summary.policy_review_items > 0 {
        return DiffNetPosture::ReviewRequired;
    }
    if summary.removed_findings > 0 || summary.policy_improvements > 0 {
        return DiffNetPosture::Improved;
    }
    DiffNetPosture::Unchanged
}

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
        "| Current no-new failures | {} |\n",
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
    out
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

pub fn render_diff_json_with_posture(report_json: &str, report: DiffReport<'_>) -> Option<String> {
    let diff_json = render_diff_posture_json(report);
    let trimmed = report_json.trim_end();
    trimmed
        .strip_suffix('}')
        .map(|prefix| format!("{prefix},\n  \"diff\": {diff_json}\n}}\n"))
}

fn render_diff_posture_json(report: DiffReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "    \"net_posture\": \"{}\",\n",
        json_escape(report.net_posture)
    ));
    out.push_str(&format!(
        "    \"reviewer_action\": \"{}\",\n",
        json_escape(report.reviewer_action)
    ));
    out.push_str("    \"summary\": {\n");
    out.push_str(&format!(
        "      \"current_failures\": {},\n",
        report.summary.current_failures
    ));
    out.push_str(&format!(
        "      \"new_findings\": {},\n",
        report.summary.new_findings
    ));
    out.push_str(&format!(
        "      \"removed_findings\": {},\n",
        report.summary.removed_findings
    ));
    out.push_str(&format!(
        "      \"policy_failures\": {},\n",
        report.summary.policy_failures
    ));
    out.push_str(&format!(
        "      \"policy_review_items\": {},\n",
        report.summary.policy_review_items
    ));
    out.push_str(&format!(
        "      \"policy_improvements\": {}\n",
        report.summary.policy_improvements
    ));
    out.push_str("    },\n");
    out.push_str("    \"finding_changes\": [\n");
    for (index, change) in report.finding_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"change\": \"{}\", ", json_escape(change.change)));
        out.push_str(&format!("\"key\": \"{}\", ", json_escape(change.key)));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"family\": {}, ", option_json(change.family)));
        out.push_str(&format!("\"path\": \"{}\"", json_escape(change.path)));
        out.push('}');
    }
    out.push_str("\n    ],\n");
    out.push_str("    \"policy_changes\": [\n");
    for (index, change) in report.policy_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!(
            "\"severity\": \"{}\", ",
            json_escape(change.severity)
        ));
        out.push_str(&format!(
            "\"allow_id\": \"{}\", ",
            json_escape(change.allow_id)
        ));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"message\": \"{}\"", json_escape(change.message)));
        out.push('}');
    }
    out.push_str("\n    ]\n");
    out.push_str("  }");
    out
}
