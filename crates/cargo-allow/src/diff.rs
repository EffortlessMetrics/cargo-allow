use allow_core::{CargoAllowResult, MatchOutcome, json_escape, normalize_path};
use allow_match::{CheckMode, evaluate};
use clap::Parser;
use std::path::PathBuf;
use std::process;

use crate::{
    OutputFormat, RootArgs, git_relative_config_path, load_world, markdown_cell,
    option_json_string, parse_kind_filter, policy_baseline_debt_entries, report_config,
    source_tree_root_text, write_file,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct DiffArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Base git revision for changed-file listing.
    #[arg(long)]
    base: String,
    /// Optional head git revision.
    #[arg(long)]
    head: Option<String>,
}

pub(crate) fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let policy_path = git_relative_config_path(&root, args.config.as_deref())?;
    let base_cfg = allow_diff::policy_config_at_revision(&root, &args.base, &policy_path)?
        .unwrap_or_else(|| report_cfg.clone());
    let head_cfg_for_diff = if let Some(head) = &args.head {
        allow_diff::policy_config_at_revision(&root, head, &policy_path)?
            .unwrap_or_else(|| report_cfg.clone())
    } else {
        report_cfg.clone()
    };
    let mut base_findings = allow_diff::findings_at_revision(&root, &args.base, &base_cfg)?;
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        base_findings.retain(|finding| parsed.matches_finding(finding));
    }
    let mut head_findings_for_diff = if let Some(head) = &args.head {
        allow_diff::findings_at_revision(&root, head, &head_cfg_for_diff)?
    } else {
        findings.clone()
    };
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        head_findings_for_diff.retain(|finding| parsed.matches_finding(finding));
    }
    let finding_changes =
        allow_diff::finding_posture_changes(&base_findings, &head_findings_for_diff);
    let policy_changes =
        allow_diff::policy_changes_from_git(&root, &args.base, &policy_path, &head_cfg_for_diff)?;
    let policy_failed = policy_changes.iter().any(|change| change.severity.fails());
    let failed = outcomes.iter().any(|o| CheckMode::NoNew.fails(o.status)) || policy_failed;
    let root_text = source_tree_root_text(&root);
    let report_context = allow_report::ReportContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        baseline_debt_entries: Some(policy_baseline_debt_entries(&report_cfg)),
    };
    let mut text = match args.format {
        OutputFormat::Json => render_diff_json_with_posture(
            allow_report::render_json_with_context(
                "diff",
                &findings,
                &outcomes,
                failed,
                report_context,
            ),
            &outcomes,
            &finding_changes,
            &policy_changes,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Human => allow_report::render_human_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
    };
    if args.format == OutputFormat::Markdown {
        let summary = render_diff_pr_summary_markdown(&outcomes, &finding_changes, &policy_changes);
        insert_markdown_pr_summary(&mut text, &summary);
    }
    append_finding_posture_changes(&mut text, args.format, &finding_changes);
    append_policy_changes(&mut text, args.format, &policy_changes);
    match allow_diff::changed_files(&root, &args.base, args.head.as_deref()) {
        Ok(changed) => {
            if args.format == OutputFormat::Human {
                text.push_str("\nChanged files from git diff:\n");
                for path in changed.iter().take(80) {
                    text.push_str(&format!("  {}\n", normalize_path(path)));
                }
            }
        }
        Err(err) => {
            if args.format == OutputFormat::Human {
                text.push_str(&format!("\nwarning: could not compute git diff: {err}\n"));
            }
        }
    }
    if args.format == OutputFormat::Json && !policy_changes.is_empty() {
        eprintln!("{}", render_policy_changes_human(&policy_changes));
    }
    if args.format == OutputFormat::Json && !finding_changes.is_empty() {
        eprintln!("{}", render_finding_posture_changes_human(&finding_changes));
    }
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

fn insert_markdown_pr_summary(text: &mut String, summary: &str) {
    let marker = "Findings scanned:";
    if let Some(index) = text.find(marker) {
        text.insert_str(index, summary);
    } else {
        text.push('\n');
        text.push_str(summary);
    }
}

fn render_diff_pr_summary_markdown(
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

fn append_finding_posture_changes(
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
    let diff_json = render_diff_posture_json(outcomes, finding_changes, policy_changes);
    let trimmed = report_json.trim_end();
    if let Some(prefix) = trimmed.strip_suffix('}') {
        format!("{prefix},\n  \"diff\": {diff_json}\n}}\n")
    } else {
        eprintln!("warning: failed to append diff posture to JSON report");
        report_json
    }
}

fn render_diff_posture_json(
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let summary = diff_posture_summary(outcomes, finding_changes, policy_changes);
    let posture = summary.net_posture();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("    \"net_posture\": \"{}\",\n", posture.as_str()));
    out.push_str(&format!(
        "    \"reviewer_action\": \"{}\",\n",
        json_escape(posture.reviewer_action())
    ));
    out.push_str("    \"summary\": {\n");
    out.push_str(&format!(
        "      \"current_failures\": {},\n",
        summary.current_failures
    ));
    out.push_str(&format!(
        "      \"new_findings\": {},\n",
        summary.new_findings
    ));
    out.push_str(&format!(
        "      \"removed_findings\": {},\n",
        summary.removed_findings
    ));
    out.push_str(&format!(
        "      \"policy_failures\": {},\n",
        summary.policy_failures
    ));
    out.push_str(&format!(
        "      \"policy_review_items\": {},\n",
        summary.policy_review_items
    ));
    out.push_str(&format!(
        "      \"policy_improvements\": {}\n",
        summary.policy_improvements
    ));
    out.push_str("    },\n");
    out.push_str("    \"finding_changes\": [\n");
    for (index, change) in finding_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"change\": \"{}\", ", change.kind.as_str()));
        out.push_str(&format!("\"key\": \"{}\", ", json_escape(&change.key)));
        out.push_str(&format!(
            "\"kind\": \"{}\", ",
            json_escape(&change.finding_kind)
        ));
        out.push_str(&format!(
            "\"family\": {}, ",
            option_json_string(change.family.as_deref())
        ));
        out.push_str(&format!("\"path\": \"{}\"", json_escape(&change.path)));
        out.push('}');
    }
    out.push_str("\n    ],\n");
    out.push_str("    \"policy_changes\": [\n");
    for (index, change) in policy_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"severity\": \"{}\", ", change.severity.as_str()));
        out.push_str(&format!(
            "\"allow_id\": \"{}\", ",
            json_escape(&change.allow_id)
        ));
        out.push_str(&format!("\"kind\": \"{}\", ", change.kind.as_str()));
        out.push_str(&format!(
            "\"message\": \"{}\"",
            json_escape(&change.message)
        ));
        out.push('}');
    }
    out.push_str("\n    ]\n");
    out.push_str("  }");
    out
}

fn render_finding_posture_changes_human(changes: &[allow_diff::FindingPostureChange]) -> String {
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

fn append_policy_changes(
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

fn render_policy_changes_human(changes: &[allow_diff::PolicyChange]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::MatchStatus;

    #[test]
    fn markdown_pr_summary_reports_unchanged_posture() {
        let text = render_diff_pr_summary_markdown(&[], &[], &[]);

        assert!(text.contains("**Net posture:** `unchanged`"));
        assert!(text.contains("| Current no-new failures | 0 |"));
        assert!(text.contains("no source exception posture change detected"));
    }

    #[test]
    fn markdown_pr_summary_reports_review_required_for_new_source_finding() {
        let changes = vec![finding_posture_change(
            allow_diff::FindingPostureKind::New,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )];

        let text = render_diff_pr_summary_markdown(&[], &changes, &[]);

        assert!(text.contains("**Net posture:** `review-required`"));
        assert!(text.contains("| New source findings | 1 |"));
        assert!(text.contains("review the source exception posture change"));
    }

    #[test]
    fn markdown_pr_summary_reports_worse_for_policy_failure() {
        let changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Fail,
            allow_diff::PolicyChangeKind::ScopeBroadened,
        )];

        let text = render_diff_pr_summary_markdown(&[], &[], &changes);

        assert!(text.contains("**Net posture:** `worse`"));
        assert!(text.contains("| Policy failures | 1 |"));
        assert!(text.contains("block until failing source exception changes"));
    }

    #[test]
    fn markdown_pr_summary_reports_improved_for_removed_source_finding() {
        let changes = vec![finding_posture_change(
            allow_diff::FindingPostureKind::Removed,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )];

        let text = render_diff_pr_summary_markdown(&[], &changes, &[]);

        assert!(text.contains("**Net posture:** `improved`"));
        assert!(text.contains("| Removed source findings | 1 |"));
        assert!(text.contains("keep the narrower posture"));
    }

    #[test]
    fn markdown_pr_summary_reports_improved_for_removed_policy_entry() {
        let changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Improvement,
            allow_diff::PolicyChangeKind::RemovedAllow,
        )];

        let text = render_diff_pr_summary_markdown(&[], &[], &changes);

        assert!(text.contains("**Net posture:** `improved`"));
        assert!(text.contains("| Policy improvements | 1 |"));
        assert!(text.contains("keep the narrower posture"));
    }

    #[test]
    fn json_report_includes_structured_posture_changes() {
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap at src/lib.rs:1:1",
        )];
        let finding_changes = vec![finding_posture_change(
            allow_diff::FindingPostureKind::New,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )];
        let policy_changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Fail,
            allow_diff::PolicyChangeKind::ScopeBroadened,
        )];

        let json = render_diff_json_with_posture(
            "{\n  \"schema_id\": \"cargo-allow.report.v1\"\n}".to_string(),
            &outcomes,
            &finding_changes,
            &policy_changes,
        );

        assert!(json.contains("\"diff\""));
        assert!(json.contains("\"net_posture\": \"worse\""));
        assert!(json.contains("\"current_failures\": 1"));
        assert!(json.contains("\"new_findings\": 1"));
        assert!(json.contains("\"policy_failures\": 1"));
        assert!(json.contains("\"policy_improvements\": 0"));
        assert!(json.contains("\"finding_changes\""));
        assert!(json.contains("\"change\": \"new\""));
        assert!(json.contains("\"family\": \"unwrap\""));
        assert!(json.contains("\"policy_changes\""));
        assert!(json.contains("\"severity\": \"fail\""));
        assert!(json.contains("\"kind\": \"scope_broadened\""));
        assert!(json.ends_with("}\n"));
    }

    #[test]
    fn json_report_keeps_base_report_when_append_fails() {
        let base = "not json".to_string();

        let json = render_diff_json_with_posture(base.clone(), &[], &[], &[]);

        assert_eq!(json, base);
    }

    fn test_outcome(
        status: MatchStatus,
        allow_id: Option<&str>,
        finding_index: Option<usize>,
        message: &str,
    ) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: allow_id.map(str::to_string),
            finding_index,
            message: message.to_string(),
            score: 100,
        }
    }

    fn finding_posture_change(
        kind: allow_diff::FindingPostureKind,
        finding_kind: &str,
        family: Option<&str>,
        path: &str,
    ) -> allow_diff::FindingPostureChange {
        allow_diff::FindingPostureChange {
            kind,
            key: format!("{finding_kind}:{path}"),
            finding_kind: finding_kind.to_string(),
            family: family.map(str::to_string),
            path: path.to_string(),
        }
    }

    fn policy_change(
        severity: allow_diff::PolicyChangeSeverity,
        kind: allow_diff::PolicyChangeKind,
    ) -> allow_diff::PolicyChange {
        allow_diff::PolicyChange {
            allow_id: "allow-0001".to_string(),
            kind,
            severity,
            message: "allow-0001 changed".to_string(),
        }
    }
}
