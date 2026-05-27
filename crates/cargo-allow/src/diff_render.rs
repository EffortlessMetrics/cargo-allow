use allow_core::MatchOutcome;
use allow_match::CheckMode;

use crate::{OutputFormat, markdown_cell};

pub(super) fn insert_markdown_pr_summary(text: &mut String, summary: &str) {
    let marker = "Findings scanned:";
    if let Some(index) = text.find(marker) {
        text.insert_str(index, summary);
    } else {
        text.push('\n');
        text.push_str(summary);
    }
}

pub(super) fn render_diff_pr_summary_markdown(
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let summary = diff_posture_summary(outcomes, finding_changes, policy_changes);
    let posture = summary.net_posture();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffPostureSummary {
    current_failures: usize,
    new_findings: usize,
    removed_findings: usize,
    policy_failures: usize,
    policy_review_items: usize,
    policy_improvements: usize,
}

impl DiffPostureSummary {
    fn net_posture(self) -> DiffNetPosture {
        diff_net_posture(
            self.current_failures,
            self.new_findings,
            self.removed_findings,
            self.policy_failures,
            self.policy_review_items,
            self.policy_improvements,
        )
    }
}

fn diff_posture_summary(
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> DiffPostureSummary {
    DiffPostureSummary {
        current_failures: outcomes
            .iter()
            .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
            .count(),
        new_findings: finding_changes
            .iter()
            .filter(|change| change.kind == allow_diff::FindingPostureKind::New)
            .count(),
        removed_findings: finding_changes
            .iter()
            .filter(|change| change.kind == allow_diff::FindingPostureKind::Removed)
            .count(),
        policy_failures: policy_changes
            .iter()
            .filter(|change| change.severity == allow_diff::PolicyChangeSeverity::Fail)
            .count(),
        policy_review_items: policy_changes
            .iter()
            .filter(|change| change.severity == allow_diff::PolicyChangeSeverity::Review)
            .count(),
        policy_improvements: policy_changes
            .iter()
            .filter(|change| change.severity == allow_diff::PolicyChangeSeverity::Improvement)
            .count(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffNetPosture {
    Worse,
    ReviewRequired,
    Improved,
    Unchanged,
}

impl DiffNetPosture {
    fn as_str(self) -> &'static str {
        match self {
            Self::Worse => "worse",
            Self::ReviewRequired => "review-required",
            Self::Improved => "improved",
            Self::Unchanged => "unchanged",
        }
    }

    fn reviewer_action(self) -> &'static str {
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

fn diff_net_posture(
    current_failures: usize,
    new_findings: usize,
    removed_findings: usize,
    policy_failures: usize,
    policy_review_items: usize,
    policy_improvements: usize,
) -> DiffNetPosture {
    if current_failures > 0 || policy_failures > 0 {
        return DiffNetPosture::Worse;
    }
    if new_findings > 0 || policy_review_items > 0 {
        return DiffNetPosture::ReviewRequired;
    }
    if removed_findings > 0 || policy_improvements > 0 {
        return DiffNetPosture::Improved;
    }
    DiffNetPosture::Unchanged
}

pub(super) fn append_finding_posture_changes(
    text: &mut String,
    format: OutputFormat,
    changes: &[allow_diff::FindingPostureChange],
) {
    match format {
        OutputFormat::Human => text.push_str(&render_finding_posture_changes_human(changes)),
        OutputFormat::Markdown => text.push_str(&render_finding_posture_changes_markdown(changes)),
        OutputFormat::Html | OutputFormat::Json | OutputFormat::Sarif => {}
    }
}

pub(crate) fn render_diff_json_with_posture(
    report_json: String,
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let summary = diff_posture_summary(outcomes, finding_changes, policy_changes);
    let posture = summary.net_posture();
    let finding_rows = finding_changes
        .iter()
        .map(|change| allow_report::DiffFindingChange {
            change: change.kind.as_str(),
            key: &change.key,
            kind: &change.finding_kind,
            family: change.family.as_deref(),
            path: &change.path,
        })
        .collect::<Vec<_>>();
    let policy_rows = policy_changes
        .iter()
        .map(|change| allow_report::DiffPolicyChange {
            severity: change.severity.as_str(),
            allow_id: &change.allow_id,
            kind: change.kind.as_str(),
            message: &change.message,
        })
        .collect::<Vec<_>>();
    let report = allow_report::DiffReport {
        net_posture: posture.as_str(),
        reviewer_action: posture.reviewer_action(),
        summary: allow_report::DiffPostureSummary {
            current_failures: summary.current_failures,
            new_findings: summary.new_findings,
            removed_findings: summary.removed_findings,
            policy_failures: summary.policy_failures,
            policy_review_items: summary.policy_review_items,
            policy_improvements: summary.policy_improvements,
        },
        finding_changes: &finding_rows,
        policy_changes: &policy_rows,
    };
    if let Some(json) = allow_report::render_diff_json_with_posture(&report_json, report) {
        json
    } else {
        eprintln!("warning: failed to append diff posture to JSON report");
        report_json
    }
}

pub(super) fn render_finding_posture_changes_human(
    changes: &[allow_diff::FindingPostureChange],
) -> String {
    let mut out = String::new();
    out.push_str("\nFinding posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    for change in changes.iter().take(120) {
        out.push_str(&format!(
            "  {} {}{} at {}\n",
            change.kind.as_str(),
            change.finding_kind,
            change
                .family
                .as_ref()
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

fn render_finding_posture_changes_markdown(changes: &[allow_diff::FindingPostureChange]) -> String {
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
            markdown_cell(change.kind.as_str()),
            markdown_cell(&change.finding_kind),
            markdown_cell(change.family.as_deref().unwrap_or("")),
            markdown_cell(&change.path)
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

pub(super) fn append_policy_changes(
    text: &mut String,
    format: OutputFormat,
    changes: &[allow_diff::PolicyChange],
) {
    match format {
        OutputFormat::Human => text.push_str(&render_policy_changes_human(changes)),
        OutputFormat::Markdown => text.push_str(&render_policy_changes_markdown(changes)),
        OutputFormat::Html | OutputFormat::Json | OutputFormat::Sarif => {}
    }
}

pub(super) fn render_policy_changes_human(changes: &[allow_diff::PolicyChange]) -> String {
    let mut out = String::new();
    out.push_str("\nPolicy posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    for change in changes {
        out.push_str(&format!(
            "  {} {} {}: {}\n",
            change.severity.as_str(),
            change.allow_id,
            change.kind.as_str(),
            change.message
        ));
    }
    out
}

fn render_policy_changes_markdown(changes: &[allow_diff::PolicyChange]) -> String {
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
            markdown_cell(change.severity.as_str()),
            markdown_cell(&change.allow_id),
            markdown_cell(change.kind.as_str()),
            markdown_cell(&change.message)
        ));
    }
    out
}
