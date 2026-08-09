use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding, MatchOutcome,
    normalize_path, read_text_file_capped, source_tree_path_is_ignored,
};
use allow_inventory::{InventorySource, resolve_source_tree_root};
use allow_match::{CheckMode, evaluate};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[path = "diff_args.rs"]
mod diff_args;
#[path = "diff_render.rs"]
mod diff_render;
#[path = "diff_row.rs"]
mod diff_row;
pub(crate) use diff_args::DiffArgs;
pub(crate) use diff_render::DiffLedgerContext;
#[cfg(test)]
pub(crate) use diff_render::render_diff_json_with_posture;
use diff_render::{
    append_diff_posture_summary_styled, append_finding_posture_changes_styled,
    append_policy_changes_styled, insert_markdown_pr_summary, render_diff_json_report,
    render_diff_pr_summary_markdown, render_finding_posture_changes_human,
    render_policy_changes_human,
};

use crate::{
    EvidenceReportSummary, EvidenceValidationMode, InventoryFacts, OutputFormat,
    SourceTreeReportContext, assert_path_within_root, current_dir, emit_text,
    git_relative_config_path, load_world_with_evidence_mode, parse_kind_filter,
    policy_baseline_debt_entries, report_config, write_file,
};

struct CurrentWorld {
    root: PathBuf,
    cfg: AllowConfig,
    findings: Vec<Finding>,
    inventory_facts: InventoryFacts,
}

pub(crate) fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
    // Auto-detect merge-base when --base is omitted (#2788).
    let base = match &args.base {
        Some(base) => base.clone(),
        None => {
            let root = resolve_diff_root(args.root.root.as_deref())?;
            auto_detect_merge_base(&root)?.ok_or_else(|| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Usage,
                    "diff requires --base <revision>; \
                     no upstream was auto-detected for merge-base; \
                     pass --base origin/main or set an upstream with `git branch --set-upstream-to`",
                )
            })?
        }
    };
    let current_world = if args.head.is_none() {
        Some(load_current_world(args)?)
    } else {
        None
    };
    let root = current_world
        .as_ref()
        .map(|world| world.root.clone())
        .map(Ok)
        .unwrap_or_else(|| resolve_diff_root(args.root.root.as_deref()))?;
    if let Some(receipt) = &args.receipt {
        assert_path_within_root(&root, receipt)?;
    }
    let policy_path = git_relative_config_path_for_diff(
        &root,
        args.config.as_deref(),
        &base,
        args.head.as_deref(),
    )?;
    let base_cfg = allow_diff::policy_config_at_revision(&root, &base, &policy_path)?
        .unwrap_or_else(AllowConfig::empty);
    let head_cfg_for_diff = if let Some(head) = &args.head {
        allow_diff::policy_config_at_revision(&root, head, &policy_path)?
            .unwrap_or_else(AllowConfig::empty)
    } else {
        current_world_loaded(&current_world)?.cfg.clone()
    };
    let base_revision_scan = allow_diff::scan_at_revision(&root, &base, &base_cfg)?;
    let mut base_findings = base_revision_scan.findings.clone();
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        base_findings.retain(|finding| parsed.matches_finding(finding));
    }
    let head_revision_scan = if let Some(head) = &args.head {
        Some(allow_diff::scan_at_revision(
            &root,
            head,
            &head_cfg_for_diff,
        )?)
    } else {
        None
    };
    let mut head_findings_for_diff = if let Some(scan) = &head_revision_scan {
        scan.findings.clone()
    } else {
        current_world_loaded(&current_world)?.findings.clone()
    };
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        head_findings_for_diff.retain(|finding| parsed.matches_finding(finding));
    }
    let evidence_source_tree_files = if let Some(revision) = args.head.as_deref() {
        Some(source_tree_files_at_revision(&root, revision)?)
    } else {
        crate::evidence_inventory::current_evidence_source_tree_files(&root, args.include_untracked)
    };
    let report_cfg = if args.head.is_some() {
        report_config(&head_cfg_for_diff, args.kind.as_deref())?
    } else {
        report_config(
            &current_world_loaded(&current_world)?.cfg,
            args.kind.as_deref(),
        )?
    };
    let findings_for_report = if args.head.is_some() {
        head_findings_for_diff.clone()
    } else {
        current_world_loaded(&current_world)?.findings.clone()
    };
    let outcomes = evaluate(&report_cfg, &findings_for_report, CheckMode::NoNew);
    let projected_outcomes = allow_report::ledger_project_outcomes(
        &report_cfg,
        &outcomes,
        allow_core::SimpleDate::today_utc_approx(),
    );
    let head_coverage = if let Some(scan) = &head_revision_scan {
        allow_diff::DiffScanCoverage {
            inventory_complete: scan.inventory_completeness == "complete",
            scanner_complete: scan.rust_files_skipped == 0
                && scan.rust_files_with_parse_errors == 0,
        }
    } else {
        // Current-tree completeness is the separate #2493 lane. This slice
        // classifies only the two exact committed revisions when --head is
        // supplied; existing current-tree checks continue to enforce their
        // own inventory and scanner failure rules.
        allow_diff::DiffScanCoverage::complete()
    };
    let base_inventory_complete = base_revision_scan.inventory_completeness == "complete";
    let base_scanner_complete = base_revision_scan.rust_files_skipped == 0
        && base_revision_scan.rust_files_with_parse_errors == 0;
    let result_class = allow_diff::classify_diff_result(
        allow_diff::DiffScanCoverage {
            inventory_complete: base_inventory_complete,
            scanner_complete: base_scanner_complete,
        },
        head_coverage,
    );
    let finding_changes = allow_diff::retain_confident_finding_changes(
        result_class,
        allow_diff::finding_posture_changes(&base_findings, &head_findings_for_diff),
    );
    let head_inventory_complete = head_coverage.inventory_complete;
    let head_scanner_complete = head_coverage.scanner_complete;
    let mut policy_changes = policy_changes_for_diff(
        Some(base_cfg.clone()),
        &head_cfg_for_diff,
        args.kind.as_deref(),
    )?;
    promote_broken_added_local_reference_policy_changes(
        &root,
        evidence_source_tree_files.as_ref(),
        &head_cfg_for_diff,
        &mut policy_changes,
    )?;
    let policy_failed = policy_changes.iter().any(|change| change.severity.fails());
    // #2075: when --require-change-note is set, every weakening policy edit
    // (severity Fail or Review) must have a matching revision note in the
    // revisions dir. Missing notes produce a blocking diagnostic naming the
    // allow_id + change_kind.
    let missing_change_notes = if args.require_change_note {
        check_change_notes(
            &root,
            &args.revisions_dir,
            &base_cfg,
            &head_cfg_for_diff,
            &policy_changes,
        )?
    } else {
        Vec::new()
    };
    let change_note_failed = !missing_change_notes.is_empty();
    if let Some(template_path) = args.write_change_note_template.as_deref() {
        if !args.require_change_note {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--write-change-note-template requires --require-change-note",
            ));
        }
        write_change_note_template(&root, template_path, &missing_change_notes)?;
        eprintln!(
            "change note template written: {}",
            root.join(template_path).display()
        );
    }
    let evidence = evidence_summary_for_diff(
        &root,
        evidence_source_tree_files.as_ref(),
        &report_cfg,
        &outcomes,
    );
    let current_failures = projected_outcomes
        .iter()
        .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
        .count()
        + evidence.broken_evidence_links;
    let current_failures =
        current_failures + usize::from(result_class.is_blocking() && current_failures == 0);
    // A required note is the authorization for a bounded weakening. Without
    // the flag, preserve the ordinary posture failure. With the flag, only a
    // missing or stale note keeps the diff blocked.
    let policy_change_failure = policy_failed && (!args.require_change_note || change_note_failed);
    let failed = current_failures > 0 || policy_change_failure || change_note_failed;
    let report_inventory_facts = if let Some(head) = args.head.as_deref() {
        let mut facts = InventoryFacts::scanned(
            InventorySource::GitTracked,
            source_tree_file_count_at_revision(&root, head, &report_cfg)?,
        );
        if let Some(scan) = &head_revision_scan {
            facts = facts
                .with_rust_files_considered(scan.rust_files_considered)
                .with_rust_files_skipped(scan.rust_files_skipped)
                .with_rust_files_with_parse_errors(scan.rust_files_with_parse_errors);
        }
        facts
    } else {
        current_world_loaded(&current_world)?.inventory_facts
    };
    let source_context = SourceTreeReportContext::new(&root, report_inventory_facts);
    let mut report_context = source_context.report(Some(policy_baseline_debt_entries(&report_cfg)));
    evidence.apply_to(&mut report_context);
    let provisional_ledger = DiffLedgerContext::new(
        &base_cfg,
        &head_cfg_for_diff,
        &finding_changes,
        &policy_changes,
        allow_report::DiffAnalysisContext::default(),
    );
    let movement = provisional_ledger.ledger_movement_summary().movement;
    let diff_analysis = allow_report::DiffAnalysisContext {
        result_class: result_class.as_str(),
        base_revision: Some(&base),
        head_revision: args.head.as_deref(),
        base_inventory_complete,
        base_scanner_complete,
        head_inventory_complete,
        head_scanner_complete,
        introduced: movement.introduced,
        retained: movement.retained,
        removed: movement.removed,
    };
    report_context.diff_analysis = Some(diff_analysis);
    let receipt_context = report_context;
    let ledger = DiffLedgerContext::new(
        &base_cfg,
        &head_cfg_for_diff,
        &finding_changes,
        &policy_changes,
        diff_analysis,
    );
    let mut text = match args.format {
        OutputFormat::Json => render_diff_json_report(
            &findings_for_report,
            &projected_outcomes,
            failed,
            report_context,
            current_failures,
            &ledger,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            "diff",
            &findings_for_report,
            &projected_outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            "diff",
            &findings_for_report,
            &projected_outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            "diff",
            &findings_for_report,
            &projected_outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Human => allow_report::render_human_with_context(
            "diff",
            &findings_for_report,
            &projected_outcomes,
            failed,
            report_context,
        ),
    };
    if args.format == OutputFormat::Markdown {
        let summary = render_diff_pr_summary_markdown(
            current_failures,
            evidence,
            &projected_outcomes,
            &ledger,
        );
        insert_markdown_pr_summary(&mut text, &summary);
    }
    let style = if args.format == OutputFormat::Human && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    append_diff_posture_summary_styled(
        &mut text,
        args.format,
        current_failures,
        evidence,
        &projected_outcomes,
        &ledger,
        style,
    );
    append_finding_posture_changes_styled(
        &mut text,
        args.format,
        &finding_changes,
        &head_cfg_for_diff,
        style,
    );
    append_policy_changes_styled(
        &mut text,
        args.format,
        &policy_changes,
        &head_cfg_for_diff,
        style,
    );
    let summary = diff_summary(
        &base,
        args.head.as_deref(),
        result_class,
        current_failures,
        failed,
    )?;
    crate::core_command_router::write_summary_artifact(&root, &summary)?;
    if args.format == OutputFormat::Human {
        text = format!(
            "{}\n{text}",
            crate::core_command_summary::render_core_command_summary_human(&summary)
        );
    }
    match allow_diff::changed_files(&root, &base, args.head.as_deref()) {
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
    // Suppress human-readable diagnostics on stderr when format is JSON to avoid
    // corrupting JSON pipeline scripts (#3218).
    if args.format != OutputFormat::Json {
        if result_class.is_blocking() {
            eprintln!(
                "diff result: {} (non-complete evidence is non-clean; repair the indicated revision input before relying on movement)",
                result_class.as_str()
            );
        }
        if !policy_changes.is_empty() {
            eprintln!(
                "{}",
                render_policy_changes_human(&policy_changes, &head_cfg_for_diff)
            );
        }
        if !finding_changes.is_empty() {
            eprintln!(
                "{}",
                render_finding_posture_changes_human(&finding_changes, &head_cfg_for_diff)
            );
        }
    }
    // Surface missing change notes as a clear stderr diagnostic (#2075).
    // Guarded by format check so JSON pipelines see clean stderr (#3218).
    if args.format != OutputFormat::Json {
        for missing in &missing_change_notes {
            let fingerprint_hint = format_fingerprint_hint(
                missing.before_fingerprint.as_deref(),
                missing.after_fingerprint.as_deref(),
            );
            eprintln!(
                "change note required: {allow_id} {kind} ({severity}) — add a revision note in {dir} \
             covering allow_id=\"{allow_id}\" change_kind=\"{kind}\"",
                allow_id = missing.allow_id,
                kind = missing.change_kind,
                severity = missing.severity,
                dir = args.revisions_dir.display()
            );
            if !fingerprint_hint.is_empty() {
                eprintln!("change note fingerprint template:{fingerprint_hint}");
            }
        }
    }
    emit_text(args.output.as_deref(), &text)?;
    if let Some(path) = &args.receipt {
        let receipt = allow_report::render_receipt_with_context_and_inventory(
            "diff",
            &findings_for_report,
            &projected_outcomes,
            failed,
            receipt_context,
        );
        write_file(path, &receipt)?;
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

fn diff_summary(
    base: &str,
    head: Option<&str>,
    result_class: allow_diff::DiffResultClass,
    current_failures: usize,
    failed: bool,
) -> CargoAllowResult<crate::core_command_summary::CoreCommandSummaryV1> {
    use effortless_repo_protocol::{CompletenessV1, CurrentnessV1, ResultClassV1};
    let (result_class_v1, completeness, currentness) = match result_class {
        allow_diff::DiffResultClass::Complete => (
            ResultClassV1::Completed,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
        ),
        allow_diff::DiffResultClass::StaleInput => (
            ResultClassV1::StaleInput,
            CompletenessV1::Unknown,
            CurrentnessV1::Stale,
        ),
        allow_diff::DiffResultClass::Unsupported => (
            ResultClassV1::Unsupported,
            CompletenessV1::Unknown,
            CurrentnessV1::PartialOrUnavailable,
        ),
        allow_diff::DiffResultClass::InstrumentFailure => (
            ResultClassV1::InstrumentFailure,
            CompletenessV1::Unknown,
            CurrentnessV1::PartialOrUnavailable,
        ),
        allow_diff::DiffResultClass::BasePartial
        | allow_diff::DiffResultClass::HeadPartial
        | allow_diff::DiffResultClass::BothPartial => (
            ResultClassV1::PartialData,
            CompletenessV1::Partial,
            CurrentnessV1::PartialOrUnavailable,
        ),
    };
    crate::core_command_summary::core_command_summary_from_diff(
        crate::core_command_summary::DiffSummaryFactsV1 {
            repository_identity: "local-repository:current".to_string(),
            portable_identity: format!("diff:{base}:{}", head.unwrap_or("current-worktree")),
            base: base.to_string(),
            head: head.map(str::to_string),
            result_class: result_class_v1,
            completeness,
            currentness,
            current_failures,
            failed,
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build diff command summary: {error}"),
        )
    })
}

fn load_current_world(args: &DiffArgs) -> CargoAllowResult<CurrentWorld> {
    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    Ok(CurrentWorld {
        root,
        cfg,
        findings,
        inventory_facts,
    })
}

fn current_world_loaded(current_world: &Option<CurrentWorld>) -> CargoAllowResult<&CurrentWorld> {
    current_world.as_ref().ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "internal error: current diff world was not loaded",
        )
    })
}

fn resolve_diff_root(explicit_root: Option<&Path>) -> CargoAllowResult<PathBuf> {
    let cwd = current_dir()?;
    resolve_source_tree_root(explicit_root, cwd)
}

/// Auto-detect the merge-base of HEAD and its upstream branch (@{u}).
/// Returns Ok(None) if no upstream is configured (detached HEAD, no @{u}).
/// Returns Err if git cannot be spawned (e.g. not on PATH).
fn auto_detect_merge_base(root: &Path) -> CargoAllowResult<Option<String>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("merge-base")
        .arg("HEAD")
        .arg("@{u}")
        .output()
        .map_err(|e| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Inventory,
                format!("failed to run git merge-base: {e}"),
            )
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    let oid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if oid.is_empty() {
        Ok(None)
    } else {
        Ok(Some(oid))
    }
}

fn git_relative_config_path_for_diff(
    root: &Path,
    config: Option<&Path>,
    base: &str,
    head: Option<&str>,
) -> CargoAllowResult<PathBuf> {
    if head.is_none() {
        return git_relative_config_path(root, config);
    }
    let head = head.unwrap_or(base);
    if let Some(config) = config {
        let path = explicit_diff_config_path(root, config)?;
        if revision_has_config(root, head, &path)? || revision_has_config(root, base, &path)? {
            return Ok(path);
        }
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "policy config {} not found in compared revisions",
                normalize_path(&path)
            ),
        ));
    }
    for candidate in default_diff_policy_paths() {
        let candidate = PathBuf::from(candidate);
        if revision_has_config(root, head, &candidate)? {
            return Ok(candidate);
        }
    }
    for candidate in default_diff_policy_paths() {
        let candidate = PathBuf::from(candidate);
        if revision_has_config(root, base, &candidate)? {
            return Ok(candidate);
        }
    }
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::InvalidConfig,
        "no policy config found in compared revisions; pass --config",
    ))
}

fn default_diff_policy_paths() -> [&'static str; 4] {
    [
        "policy/cargo-allow.toml",
        "policy/allow.toml",
        ".cargo/allow.toml",
        "allow.toml",
    ]
}

fn explicit_diff_config_path(root: &Path, config: &Path) -> CargoAllowResult<PathBuf> {
    if config.is_absolute() {
        return git_relative_config_path(root, Some(config));
    }
    let text = config.to_string_lossy().replace('\\', "/");
    if text.trim().is_empty() || text.trim() != text {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "explicit --config path must be source-tree-relative",
        ));
    }
    if text.starts_with('/') || text.contains(':') || text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "explicit --config path must stay inside the source tree",
        ));
    }
    let normalized = normalize_path(config);
    if normalized.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "explicit --config path must name a policy file",
        ));
    }
    Ok(PathBuf::from(normalized))
}

fn revision_has_config(root: &Path, revision: &str, path: &Path) -> CargoAllowResult<bool> {
    allow_diff::read_file_at_revision(root, revision, path).map(|text| text.is_some())
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

fn promote_broken_added_local_reference_policy_changes(
    root: &Path,
    head_files: Option<&BTreeSet<String>>,
    head_cfg: &AllowConfig,
    changes: &mut [allow_diff::PolicyChange],
) -> CargoAllowResult<()> {
    for change in changes {
        let Some(reference_kind) = AddedLocalReferenceKind::from_policy_change_kind(change.kind)
        else {
            continue;
        };
        if change.severity.fails() {
            continue;
        }
        let Some(evidence) = change.evidence.as_ref() else {
            continue;
        };
        let Some(message) = added_reference_broken_local_link_message(
            root,
            head_files,
            head_cfg,
            &change.allow_id,
            &evidence.added,
            reference_kind,
        ) else {
            continue;
        };
        change.severity = allow_diff::PolicyChangeSeverity::Fail;
        change.message = format!("{} {message}", change.allow_id);
    }
    Ok(())
}

fn added_reference_broken_local_link_message(
    root: &Path,
    head_files: Option<&BTreeSet<String>>,
    head_cfg: &AllowConfig,
    allow_id: &str,
    added: &[String],
    reference_kind: AddedLocalReferenceKind,
) -> Option<&'static str> {
    if let Some(head_files) = head_files {
        return added
            .iter()
            .filter_map(|reference| local_evidence_reference(reference))
            .find_map(|reference| {
                reference.broken_message_in_revision(head_files, reference_kind)
            });
    }
    let entry = head_cfg.allow.iter().find(|entry| entry.id == allow_id)?;
    let diagnostics = match reference_kind {
        AddedLocalReferenceKind::Evidence => {
            allow_policy::evidence_reference_diagnostics(root, entry)
        }
        AddedLocalReferenceKind::Link => {
            let mut entry = entry.clone();
            entry.evidence = added.to_vec();
            allow_policy::evidence_reference_diagnostics(root, &entry)
        }
    };
    diagnostics.iter().find_map(|diagnostic| {
        (added.iter().any(|item| item == &diagnostic.raw)
            && diagnostic.status.is_broken_local_link())
        .then_some(reference_kind.broken_message())
    })
}

fn evidence_summary_for_diff(
    root: &Path,
    head_files: Option<&BTreeSet<String>>,
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
) -> EvidenceReportSummary {
    let mut evidence = EvidenceReportSummary::from_policy(root, cfg, outcomes);
    if let Some(head_files) = head_files {
        evidence.broken_evidence_links = broken_local_reference_count_in_files(head_files, cfg);
    }
    evidence
}

fn broken_local_reference_count_in_files(
    head_files: &BTreeSet<String>,
    cfg: &AllowConfig,
) -> usize {
    cfg.allow
        .iter()
        .flat_map(|entry| entry.evidence.iter().chain(entry.links.iter()))
        .filter_map(|reference| local_evidence_reference(reference))
        .filter(|reference| reference.is_broken_in_revision(head_files))
        .count()
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

fn source_tree_file_count_at_revision(
    root: &Path,
    revision: &str,
    cfg: &AllowConfig,
) -> CargoAllowResult<usize> {
    Ok(allow_diff::git_tracked_files_at_revision(root, revision)?
        .into_iter()
        .filter(|path| !source_tree_path_is_ignored(path, &cfg.workspace.ignored))
        .count())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalEvidenceReference {
    SourceTreePath(String),
    InvalidLocalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddedLocalReferenceKind {
    Evidence,
    Link,
}

impl AddedLocalReferenceKind {
    fn from_policy_change_kind(kind: allow_diff::PolicyChangeKind) -> Option<Self> {
        match kind {
            allow_diff::PolicyChangeKind::EvidenceAdded => Some(Self::Evidence),
            allow_diff::PolicyChangeKind::LinkAdded => Some(Self::Link),
            _ => None,
        }
    }

    fn inventory_message(self) -> &'static str {
        match self {
            Self::Evidence => "local evidence added outside compared source-tree inventory",
            Self::Link => "local link added outside compared source-tree inventory",
        }
    }

    fn broken_message(self) -> &'static str {
        match self {
            Self::Evidence => "broken local evidence added",
            Self::Link => "broken local link added",
        }
    }
}

impl LocalEvidenceReference {
    fn is_broken_in_revision(&self, head_files: &BTreeSet<String>) -> bool {
        self.broken_message_in_revision(head_files, AddedLocalReferenceKind::Evidence)
            .is_some()
    }

    fn broken_message_in_revision(
        &self,
        head_files: &BTreeSet<String>,
        reference_kind: AddedLocalReferenceKind,
    ) -> Option<&'static str> {
        match self {
            Self::SourceTreePath(target) if !head_files.contains(target) => {
                Some(reference_kind.inventory_message())
            }
            Self::SourceTreePath(_) => None,
            Self::InvalidLocalPath => Some(reference_kind.broken_message()),
        }
    }
}

fn local_evidence_reference(reference: &str) -> Option<LocalEvidenceReference> {
    let (prefix, target) = reference.split_once(':')?;
    let prefix = prefix.trim();
    if !allow_policy::local_file_evidence_prefixes().any(|known| known == prefix) {
        return None;
    }
    let target = target.trim().replace('\\', "/");
    if target.is_empty()
        || target.starts_with('/')
        || target.contains(':')
        || target.split('/').any(|part| part == "." || part == "..")
        || target.chars().any(|ch| matches!(ch, '*' | '?'))
    {
        return Some(LocalEvidenceReference::InvalidLocalPath);
    }
    Some(LocalEvidenceReference::SourceTreePath(normalize_path(
        target,
    )))
}

/// A weakening policy edit that lacks a matching revision note (#2075).
struct MissingChangeNote {
    allow_id: String,
    change_kind: String,
    severity: String,
    before_fingerprint: Option<String>,
    after_fingerprint: Option<String>,
}

/// Check whether every weakening policy edit (`severity == Fail` or `Review`)
/// has a matching revision note in `revisions_dir`. Returns the list of missing
/// notes (empty when all are covered or the flag is not set).
///
/// A revision note is a `.toml` file in `revisions_dir` containing at least:
/// ```toml
/// [[records]]
/// allow_ids = ["allow-0001"]
/// change_kinds = ["scope_broadened"]
/// before_fingerprint = "sha256:v1:..."
/// after_fingerprint = "sha256:v1:..."
/// ```
/// Matching is structural on `(allow_id, change_kind, before_fingerprint,
/// after_fingerprint)` for retained entries. A note covers a change if the
/// change's identity and transition are both exact. Added entries have no
/// before fingerprint and remain keyed by `(allow_id, change_kind)`.
fn check_change_notes(
    root: &std::path::Path,
    revisions_dir: &std::path::Path,
    base_cfg: &AllowConfig,
    head_cfg: &AllowConfig,
    policy_changes: &[allow_diff::PolicyChange],
) -> CargoAllowResult<Vec<MissingChangeNote>> {
    let dir = root.join(revisions_dir);
    let mut covered: Vec<(String, String, Option<String>, Option<String>)> = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(&dir).map_err(|e| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Inventory,
                format!("read revisions dir {}: {e}", dir.display()),
            )
        })? {
            let entry = entry.map_err(|e| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Inventory,
                    format!("read dir entry: {e}"),
                )
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let text = read_text_file_capped(&path).map_err(|e| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Inventory,
                    format!("read {}: {e}", path.display()),
                )
            })?;
            let table: toml::Table = toml::from_str(&text).map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!("parse revision note {}: {error}", path.display()),
                )
            })?;
            if let Some(records) = table.get("records").and_then(|v| v.as_array()) {
                for record in records {
                    let allow_ids: Vec<String> = record
                        .get("allow_ids")
                        .and_then(|v| v.as_array())
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect();
                    let change_kinds: Vec<String> = record
                        .get("change_kinds")
                        .and_then(|v| v.as_array())
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect();
                    let before_fingerprint = record
                        .get("before_fingerprint")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    let after_fingerprint = record
                        .get("after_fingerprint")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    for aid in &allow_ids {
                        for kind in &change_kinds {
                            covered.push((
                                aid.clone(),
                                kind.clone(),
                                before_fingerprint.clone(),
                                after_fingerprint.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut missing = Vec::new();
    for change in policy_changes {
        if !matches!(
            change.severity,
            allow_diff::PolicyChangeSeverity::Fail | allow_diff::PolicyChangeSeverity::Review
        ) {
            continue;
        }
        let kind_str = change.kind.as_str();
        let before_fingerprint = base_cfg
            .allow
            .iter()
            .find(|entry| entry.id == change.allow_id)
            .map(allow_core::allow_entry_content_fingerprint);
        let after_fingerprint = head_cfg
            .allow
            .iter()
            .find(|entry| entry.id == change.allow_id)
            .map(allow_core::allow_entry_content_fingerprint);
        if !covered.iter().any(|(aid, kind, before, after)| {
            aid == &change.allow_id
                && kind == kind_str
                && before == &before_fingerprint
                && after == &after_fingerprint
        }) {
            missing.push(MissingChangeNote {
                allow_id: change.allow_id.clone(),
                change_kind: kind_str.to_string(),
                severity: change.severity.as_str().to_string(),
                before_fingerprint,
                after_fingerprint,
            });
        }
    }
    Ok(missing)
}

fn format_fingerprint_hint(before: Option<&str>, after: Option<&str>) -> String {
    let mut hint = String::new();
    if let Some(before) = before {
        hint.push_str(&format!(" before_fingerprint=\"{before}\""));
    }
    if let Some(after) = after {
        hint.push_str(&format!(" after_fingerprint=\"{after}\""));
    }
    hint
}

fn write_change_note_template(
    root: &Path,
    template_path: &Path,
    missing: &[MissingChangeNote],
) -> CargoAllowResult<()> {
    if missing.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "cannot write a change-note template when no note is required",
        ));
    }
    if template_path.is_absolute()
        || template_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "change-note template path must be repository-relative",
        ));
    }

    let destination = root.join(template_path);
    if destination.exists() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "change-note template already exists: {}",
                destination.display()
            ),
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "change-note template path has no repository parent",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "create change-note template directory {}: {error}",
                parent.display()
            ),
        )
    })?;

    let mut contents = String::from(
        "# Generated starter only. Author and review the exact transition before merge.\n\n",
    );
    for note in missing {
        contents.push_str("[[records]]\n");
        contents.push_str(&format!("allow_ids = [\"{}\"]\n", note.allow_id));
        contents.push_str(&format!("change_kinds = [\"{}\"]\n", note.change_kind));
        if let Some(before) = note.before_fingerprint.as_deref() {
            contents.push_str(&format!("before_fingerprint = \"{before}\"\n"));
        }
        if let Some(after) = note.after_fingerprint.as_deref() {
            contents.push_str(&format!("after_fingerprint = \"{after}\"\n"));
        }
        contents.push('\n');
    }

    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Scan,
                "change-note template filename must be valid UTF-8",
            )
        })?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", process::id()));
    fs::write(&temporary, contents).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "write change-note template {}: {error}",
                temporary.display()
            ),
        )
    })?;
    if let Err(error) = fs::rename(&temporary, &destination) {
        let _ = fs::remove_file(&temporary);
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "install change-note template {}: {error}",
                destination.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "diff_config_tests.rs"]
mod config_tests;
#[cfg(test)]
#[path = "diff_json_tests.rs"]
mod json_tests;
#[cfg(test)]
#[path = "diff_markdown_tests.rs"]
mod markdown_tests;
#[cfg(test)]
#[path = "diff_policy_filter_tests.rs"]
mod policy_filter_tests;

#[cfg(test)]
mod summary_tests {
    use super::diff_summary;
    use allow_diff::DiffResultClass;
    use effortless_repo_protocol::ResultClassV1;

    #[test]
    fn diff_summary_preserves_each_revision_result_class() -> Result<(), String> {
        for (result_class, expected) in [
            (DiffResultClass::Complete, ResultClassV1::Completed),
            (DiffResultClass::BasePartial, ResultClassV1::PartialData),
            (DiffResultClass::HeadPartial, ResultClassV1::PartialData),
            (DiffResultClass::BothPartial, ResultClassV1::PartialData),
            (DiffResultClass::StaleInput, ResultClassV1::StaleInput),
            (DiffResultClass::Unsupported, ResultClassV1::Unsupported),
            (
                DiffResultClass::InstrumentFailure,
                ResultClassV1::InstrumentFailure,
            ),
        ] {
            let summary = diff_summary("base", Some("head"), result_class, 1, true)?;
            if summary.result_class != expected {
                return Err(format!(
                    "{result_class:?} mapped to {:?}, expected {expected:?}",
                    summary.result_class
                ));
            }
            if summary.posture != crate::core_command_summary::CoreCommandPostureV1::Blocking {
                return Err(format!("{result_class:?} must remain blocking"));
            }
        }
        let clean = diff_summary("base", Some("head"), DiffResultClass::Complete, 0, false)?;
        if clean.result_class != ResultClassV1::Completed
            || clean.posture != crate::core_command_summary::CoreCommandPostureV1::Satisfied
        {
            return Err("complete clean diff must be satisfied".to_string());
        }
        Ok(())
    }
}
