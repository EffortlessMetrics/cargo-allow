use allow_core::{CargoAllowResult, normalize_path};
use allow_match::{CheckMode, evaluate};
use allow_policy::{broken_evidence_link_count, weak_evidence_reference_count};
use std::process;

#[path = "diff_args.rs"]
mod diff_args;
#[path = "diff_render.rs"]
mod diff_render;
pub(crate) use diff_args::DiffArgs;
#[cfg(test)]
pub(crate) use diff_render::render_diff_json_with_posture;
use diff_render::{
    append_finding_posture_changes, append_policy_changes, insert_markdown_pr_summary,
    render_diff_json_report, render_diff_pr_summary_markdown, render_finding_posture_changes_human,
    render_policy_changes_human,
};

use crate::{
    OutputFormat, SourceTreeReportContext, emit_text, git_relative_config_path,
    load_world_with_evidence_validation, matched_policy_missing_evidence_entries,
    parse_kind_filter, policy_baseline_debt_entries, report_config,
};

pub(crate) fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_validation(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
        false,
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
    let broken_evidence_links = broken_evidence_link_count(&root, &report_cfg);
    let weak_evidence_references = weak_evidence_reference_count(&root, &report_cfg);
    let current_failures = outcomes
        .iter()
        .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
        .count()
        + broken_evidence_links;
    let failed = current_failures > 0 || policy_failed;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let mut report_context = source_context.report(Some(policy_baseline_debt_entries(&report_cfg)));
    report_context.broken_evidence_links =
        (broken_evidence_links > 0).then_some(broken_evidence_links);
    report_context.weak_evidence_references =
        (weak_evidence_references > 0).then_some(weak_evidence_references);
    let policy_missing_evidence_entries =
        matched_policy_missing_evidence_entries(&report_cfg, &outcomes);
    report_context.policy_missing_evidence_entries =
        (policy_missing_evidence_entries > 0).then_some(policy_missing_evidence_entries);
    let mut text = match args.format {
        OutputFormat::Json => render_diff_json_report(
            &findings,
            &outcomes,
            failed,
            report_context,
            current_failures,
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
        let summary = render_diff_pr_summary_markdown(
            current_failures,
            &outcomes,
            &finding_changes,
            &policy_changes,
        );
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
    if args.format == OutputFormat::Json && args.output.is_none() && !policy_changes.is_empty() {
        eprintln!("{}", render_policy_changes_human(&policy_changes));
    }
    if args.format == OutputFormat::Json && args.output.is_none() && !finding_changes.is_empty() {
        eprintln!("{}", render_finding_posture_changes_human(&finding_changes));
    }
    emit_text(args.output.as_deref(), &text)?;
    if failed {
        process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
