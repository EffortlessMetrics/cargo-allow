use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowResult, Finding, MatchOutcome, normalize_path,
    source_tree_path_is_ignored,
};
use allow_inventory::{InventorySource, resolve_source_tree_root};
use allow_match::{CheckMode, evaluate};
use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process;

#[path = "diff_args.rs"]
mod diff_args;
#[path = "diff_render.rs"]
mod diff_render;
#[path = "diff_row.rs"]
mod diff_row;
pub(crate) use diff_args::DiffArgs;
#[cfg(test)]
pub(crate) use diff_render::render_diff_json_with_posture;
use diff_render::{
    append_diff_posture_summary, append_finding_posture_changes, append_policy_changes,
    insert_markdown_pr_summary, render_diff_json_report, render_diff_pr_summary_markdown,
    render_finding_posture_changes_human, render_policy_changes_human,
};

use crate::{
    EvidenceReportSummary, EvidenceValidationMode, InventoryFacts, OutputFormat,
    SourceTreeReportContext, emit_text, git_relative_config_path, load_world_with_evidence_mode,
    parse_kind_filter, policy_baseline_debt_entries, report_config,
};

struct CurrentWorld {
    root: PathBuf,
    cfg: AllowConfig,
    findings: Vec<Finding>,
    inventory_facts: InventoryFacts,
}

pub(crate) fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
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
    let policy_path = git_relative_config_path_for_diff(
        &root,
        args.config.as_deref(),
        &args.base,
        args.head.as_deref(),
    )?;
    let base_cfg = allow_diff::policy_config_at_revision(&root, &args.base, &policy_path)?
        .unwrap_or_else(AllowConfig::empty);
    let head_cfg_for_diff = if let Some(head) = &args.head {
        allow_diff::policy_config_at_revision(&root, head, &policy_path)?
            .unwrap_or_else(AllowConfig::empty)
    } else {
        current_world_loaded(&current_world)?.cfg.clone()
    };
    let mut base_findings = allow_diff::findings_at_revision(&root, &args.base, &base_cfg)?;
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        base_findings.retain(|finding| parsed.matches_finding(finding));
    }
    let mut head_findings_for_diff = if let Some(head) = &args.head {
        allow_diff::findings_at_revision(&root, head, &head_cfg_for_diff)?
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
    let finding_changes =
        allow_diff::finding_posture_changes(&base_findings, &head_findings_for_diff);
    let mut policy_changes = policy_changes_for_diff(
        allow_diff::policy_config_at_revision(&root, &args.base, &policy_path)?,
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
    let evidence = evidence_summary_for_diff(
        &root,
        evidence_source_tree_files.as_ref(),
        &report_cfg,
        &outcomes,
    );
    let current_failures = outcomes
        .iter()
        .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
        .count()
        + evidence.broken_evidence_links;
    let failed = current_failures > 0 || policy_failed;
    let report_inventory_facts = if let Some(head) = args.head.as_deref() {
        InventoryFacts::scanned(
            InventorySource::GitTracked,
            source_tree_file_count_at_revision(&root, head, &report_cfg)?,
        )
    } else {
        current_world_loaded(&current_world)?.inventory_facts
    };
    let source_context = SourceTreeReportContext::new(&root, report_inventory_facts);
    let mut report_context = source_context.report(Some(policy_baseline_debt_entries(&report_cfg)));
    evidence.apply_to(&mut report_context);
    let mut text = match args.format {
        OutputFormat::Json => render_diff_json_report(
            &findings_for_report,
            &outcomes,
            failed,
            report_context,
            current_failures,
            &finding_changes,
            &policy_changes,
            &base_cfg,
            &head_cfg_for_diff,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            "diff",
            &findings_for_report,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            "diff",
            &findings_for_report,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            "diff",
            &findings_for_report,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Human => allow_report::render_human_with_context(
            "diff",
            &findings_for_report,
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
            &base_cfg,
            &head_cfg_for_diff,
        );
        insert_markdown_pr_summary(&mut text, &summary);
    }
    append_diff_posture_summary(
        &mut text,
        args.format,
        current_failures,
        evidence,
        &outcomes,
        &finding_changes,
        &policy_changes,
        &base_cfg,
        &head_cfg_for_diff,
    );
    append_finding_posture_changes(&mut text, args.format, &finding_changes, &head_cfg_for_diff);
    append_policy_changes(&mut text, args.format, &policy_changes, &head_cfg_for_diff);
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
        eprintln!(
            "{}",
            render_policy_changes_human(&policy_changes, &head_cfg_for_diff)
        );
    }
    if args.format == OutputFormat::Json && args.output.is_none() && !finding_changes.is_empty() {
        eprintln!(
            "{}",
            render_finding_posture_changes_human(&finding_changes, &head_cfg_for_diff)
        );
    }
    emit_text(args.output.as_deref(), &text)?;
    if failed {
        process::exit(1);
    }
    Ok(())
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
    current_world
        .as_ref()
        .ok_or_else(|| CargoAllowError::new("internal error: current diff world was not loaded"))
}

fn resolve_diff_root(explicit_root: Option<&Path>) -> CargoAllowResult<PathBuf> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    resolve_source_tree_root(explicit_root, cwd)
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
        return Err(CargoAllowError::new(format!(
            "policy config {} not found in compared revisions",
            normalize_path(&path)
        )));
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
    Err(CargoAllowError::new(
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
        return Err(CargoAllowError::new(
            "explicit --config path must be source-tree-relative",
        ));
    }
    if text.starts_with('/') || text.contains(':') || text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(
            "explicit --config path must stay inside the source tree",
        ));
    }
    let normalized = normalize_path(config);
    if normalized.is_empty() {
        return Err(CargoAllowError::new(
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
