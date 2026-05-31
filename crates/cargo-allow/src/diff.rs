use allow_core::{AllowConfig, CargoAllowResult, normalize_path};
use allow_match::{CheckMode, evaluate};
use std::collections::BTreeSet;
use std::path::Path;
use std::process;

#[path = "diff_args.rs"]
mod diff_args;
#[path = "diff_render.rs"]
mod diff_render;
pub(crate) use diff_args::DiffArgs;
#[cfg(test)]
pub(crate) use diff_render::render_diff_json_with_posture;
use diff_render::{
    append_diff_posture_summary, append_finding_posture_changes, append_policy_changes,
    insert_markdown_pr_summary, render_diff_json_report, render_diff_pr_summary_markdown,
    render_finding_posture_changes_human, render_policy_changes_human,
};

use crate::{
    EvidenceReportSummary, EvidenceValidationMode, OutputFormat, SourceTreeReportContext,
    emit_text, git_relative_config_path, load_world_with_evidence_mode, parse_kind_filter,
    policy_baseline_debt_entries, report_config,
};

pub(crate) fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let policy_path = git_relative_config_path(&root, args.config.as_deref())?;
    let base_cfg = allow_diff::policy_config_at_revision(&root, &args.base, &policy_path)?
        .unwrap_or_else(AllowConfig::empty);
    let head_cfg_for_diff = if let Some(head) = &args.head {
        allow_diff::policy_config_at_revision(&root, head, &policy_path)?
            .unwrap_or_else(AllowConfig::empty)
    } else {
        cfg.clone()
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
    let mut policy_changes = policy_changes_for_diff(
        allow_diff::policy_config_at_revision(&root, &args.base, &policy_path)?,
        &head_cfg_for_diff,
        args.kind.as_deref(),
    )?;
    promote_broken_added_evidence_policy_changes(
        &root,
        args.head.as_deref(),
        &head_cfg_for_diff,
        &mut policy_changes,
    )?;
    let policy_failed = policy_changes.iter().any(|change| change.severity.fails());
    let evidence = EvidenceReportSummary::from_policy(&root, &report_cfg, &outcomes);
    let current_failures = outcomes
        .iter()
        .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
        .count()
        + evidence.broken_evidence_links;
    let failed = current_failures > 0 || policy_failed;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let mut report_context = source_context.report(Some(policy_baseline_debt_entries(&report_cfg)));
    evidence.apply_to(&mut report_context);
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
            evidence,
            &outcomes,
            &finding_changes,
            &policy_changes,
        );
        insert_markdown_pr_summary(&mut text, &summary);
    }
    append_diff_posture_summary(
        &mut text,
        args.format,
        current_failures,
        &outcomes,
        &finding_changes,
        &policy_changes,
    );
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

fn policy_changes_for_diff(
    base_cfg: Option<AllowConfig>,
    head_cfg: &AllowConfig,
    kind_filter: Option<&str>,
) -> CargoAllowResult<Vec<allow_diff::PolicyChange>> {
    let base_cfg = base_cfg.unwrap_or_else(AllowConfig::empty);
    let base_cfg = report_config(&base_cfg, kind_filter)?;
    let head_cfg = report_config(head_cfg, kind_filter)?;
    Ok(allow_diff::policy_changes(&base_cfg, &head_cfg))
}

fn promote_broken_added_evidence_policy_changes(
    root: &Path,
    head_revision: Option<&str>,
    head_cfg: &AllowConfig,
    changes: &mut [allow_diff::PolicyChange],
) -> CargoAllowResult<()> {
    let head_files = head_revision
        .map(|revision| source_tree_files_at_revision(root, revision))
        .transpose()?;
    for change in changes {
        if change.kind != allow_diff::PolicyChangeKind::EvidenceAdded || change.severity.fails() {
            continue;
        }
        let Some(evidence) = change.evidence.as_ref() else {
            continue;
        };
        if !added_evidence_has_broken_local_link(
            root,
            head_files.as_ref(),
            head_cfg,
            &change.allow_id,
            &evidence.added,
        ) {
            continue;
        }
        change.severity = allow_diff::PolicyChangeSeverity::Fail;
        change.message = format!("{} broken local evidence added", change.allow_id);
    }
    Ok(())
}

fn added_evidence_has_broken_local_link(
    root: &Path,
    head_files: Option<&BTreeSet<String>>,
    head_cfg: &AllowConfig,
    allow_id: &str,
    added: &[String],
) -> bool {
    if let Some(head_files) = head_files {
        return added
            .iter()
            .filter_map(|reference| local_evidence_target(reference))
            .any(|target| !head_files.contains(&target));
    }
    let Some(entry) = head_cfg.allow.iter().find(|entry| entry.id == allow_id) else {
        return false;
    };
    allow_policy::evidence_reference_diagnostics(root, entry)
        .iter()
        .any(|diagnostic| {
            added.iter().any(|item| item == &diagnostic.raw)
                && diagnostic.status.is_broken_local_link()
        })
}

fn source_tree_files_at_revision(
    root: &Path,
    revision: &str,
) -> CargoAllowResult<BTreeSet<String>> {
    Ok(allow_diff::git_tracked_files_at_revision(root, revision)?
        .into_iter()
        .map(normalize_path)
        .collect())
}

fn local_evidence_target(reference: &str) -> Option<String> {
    let (prefix, target) = reference.split_once(':')?;
    let prefix = prefix.trim();
    if !allow_policy::local_file_evidence_prefixes().any(|known| known == prefix) {
        return None;
    }
    Some(target.trim().replace('\\', "/"))
}

#[cfg(test)]
#[path = "diff_json_tests.rs"]
mod json_tests;
#[cfg(test)]
#[path = "diff_markdown_tests.rs"]
mod markdown_tests;
#[cfg(test)]
#[path = "diff_policy_filter_tests.rs"]
mod policy_filter_tests;
