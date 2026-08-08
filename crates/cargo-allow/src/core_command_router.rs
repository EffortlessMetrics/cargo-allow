use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, MatchStatus};
use allow_inventory::InventoryCompleteness;
use effortless_repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core_command_summary::{
    CoreCommandActionV1, CoreCommandEffectsV1, CoreCommandPostureV1, CoreCommandReasonV1,
    CoreCommandSummaryInputV1, CoreCommandSummaryV1, CoreSourceSubjectKindV1, CoreSourceSubjectV1,
    build_core_command_summary, render_core_command_summary_human,
    render_core_command_summary_json,
};
use crate::reporting::{ReportRenderArgs, SourceTreeReportContext};
use crate::{OutputFormat, emit_text, write_file};

/// The base a command resolves one of its own artifact paths against.
///
/// Commands do not agree: `--config` is discovered under the source-tree root
/// everywhere, `adopt` resolves `--output` under the root too, while the
/// commands that emit through `emit_text` (and `why --plan`) write relative to
/// the working directory. A conflict is only real when the summary sidecar and
/// the artifact land on the same file, so each candidate must be resolved
/// against the base its own command actually uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConflictBase {
    /// Resolved under the source-tree root (`--config`, `adopt --output`).
    SourceTreeRoot,
    /// Resolved under the process working directory.
    WorkingDirectory,
}

#[derive(Debug, Clone)]
pub(crate) struct SummaryOutputConfig {
    path: PathBuf,
    conflicts: Vec<(ConflictBase, PathBuf)>,
}

impl SummaryOutputConfig {
    pub(crate) fn new(path: PathBuf, conflicts: Vec<(ConflictBase, PathBuf)>) -> Self {
        Self { path, conflicts }
    }
}

static SUMMARY_OUTPUT: OnceLock<SummaryOutputConfig> = OnceLock::new();

pub(crate) fn configure_summary_output(config: SummaryOutputConfig) -> CargoAllowResult<()> {
    SUMMARY_OUTPUT.set(config).map_err(|_| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "core command summary output was configured more than once",
        )
    })
}

pub(crate) fn print_report(args: ReportRenderArgs<'_>) -> CargoAllowResult<()> {
    if !matches!(args.command, "audit" | "check") {
        return crate::reporting::print_report(args);
    }
    print_report_with_summary_config(args, SUMMARY_OUTPUT.get())
}

fn print_report_with_summary_config(
    args: ReportRenderArgs<'_>,
    summary_config: Option<&SummaryOutputConfig>,
) -> CargoAllowResult<()> {
    let summary = build_report_summary(&args)?;
    let detail = render_detail(&args);
    let rendered = if args.format == OutputFormat::Human {
        format!("{}\n{detail}", render_core_command_summary_human(&summary))
    } else {
        detail
    };

    write_summary_artifact_with_config(args.root, &summary, summary_config)?;

    emit_text(args.output, &rendered)
}

/// Write the `--summary-output` artifact for a command that builds its own
/// summary. Commands that render through [`print_report`] get this for free;
/// `adopt` and `doctor` keep their own detailed artifacts and call this
/// directly. Does nothing when `--summary-output` was not requested.
pub(crate) fn write_summary_artifact(
    root: &Path,
    summary: &CoreCommandSummaryV1,
) -> CargoAllowResult<()> {
    write_summary_artifact_with_config(root, summary, SUMMARY_OUTPUT.get())
}

fn write_summary_artifact_with_config(
    root: &Path,
    summary: &CoreCommandSummaryV1,
    summary_config: Option<&SummaryOutputConfig>,
) -> CargoAllowResult<()> {
    let Some(config) = summary_config else {
        return Ok(());
    };
    let path = validate_summary_output_path(root, config)?;
    let json = render_core_command_summary_json(summary).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to render core command summary: {error}"),
        )
    })?;
    // `write_file` returns `RepoEditError` after the effortless-repo-edit
    // product-neutrality refactor (#3283); `?` coerces it into the command
    // error type, matching the other call sites.
    Ok(write_file(&path, &format!("{json}\n"))?)
}

fn build_report_summary(args: &ReportRenderArgs<'_>) -> CargoAllowResult<CoreCommandSummaryV1> {
    let completeness = summary_completeness(&args.inventory_facts);
    let advisory_count = report_advisory_count(args);
    let (result_class, posture, reason) = if completeness != CompletenessV1::Complete {
        (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: format!("{}.partial_coverage", args.command),
                message: partial_coverage_reason(&args.inventory_facts),
            },
        )
    } else if args.failed {
        (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: format!("{}.blocking_findings", args.command),
                message: format!(
                    "{advisory_count} blocking or review outcome(s) require attention"
                ),
            },
        )
    } else if advisory_count > 0 {
        (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Advisory,
            CoreCommandReasonV1 {
                code: format!("{}.advisory_findings", args.command),
                message: format!("{advisory_count} advisory or review outcome(s) remain"),
            },
        )
    } else {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Satisfied,
            CoreCommandReasonV1 {
                code: format!("{}.satisfied", args.command),
                message: "the selected source-exception posture is satisfied".to_string(),
            },
        )
    };

    let primary_action = if completeness != CompletenessV1::Complete {
        Some(
            CoreCommandActionV1::command(
                format!("{}.diagnose_coverage", args.command),
                "Diagnose coverage",
                "cargo-allow",
                vec!["doctor".to_string()],
            )
            .with_contract(
                "coverage limitations must be diagnosed before a clean result is possible",
                "the selected inventory, scanner, policy, and support limitations are explained",
                "doctor remains read-only and does not repair or authorize exceptions",
            ),
        )
    } else if advisory_count > 0 || args.failed {
        Some(
            CoreCommandActionV1::command(
                format!("{}.inspect_worklist", args.command),
                "Inspect the worklist",
                "cargo-allow",
                vec![
                    "worklist".to_string(),
                    "--format".to_string(),
                    "json".to_string(),
                ],
            )
            .with_contract(
                "the report contains one or more exact maintenance or repair outcomes",
                "the current typed work items and their detailed actions are emitted",
                "the worklist is guidance and does not mutate source or policy",
            ),
        )
    } else {
        None
    };

    let next_proof =
        (args.command == "audit" && completeness == CompletenessV1::Complete).then(|| {
            CoreCommandActionV1::command(
                "audit.full_no_new_check",
                "Run the enforcing no-new check",
                "cargo-allow",
                vec![
                    "check".to_string(),
                    "--mode".to_string(),
                    "no-new".to_string(),
                ],
            )
            .with_contract(
                "audit is informational even when its source inputs are complete",
                "the current repository is evaluated under the no-new gate",
                "the source-syntax gate does not prove compiled or runtime correctness",
            )
        });

    let subject = report_subject(args)?;
    let mut limitations = vec![
        "cargo metadata, rustc, Clippy, build scripts, proc macros, tests, and repository code were not invoked"
            .to_string(),
        "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
            .to_string(),
    ];
    limitations.extend(subject.limitations.iter().cloned());

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: args.command.to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness,
        currentness: CurrentnessV1::Current,
        reason,
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects: CoreCommandEffectsV1::read_only(vec![
            "does not modify source, policy, Git, hooks, workflows, or GitHub settings".to_string(),
            "does not execute repository code or external evidence tools".to_string(),
        ]),
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(
            "cargo-allow evaluated selected source-tree syntax and source-exception ledger posture only",
        )
        .with_limitations(limitations),
    })
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build core command summary: {error}"),
        )
    })
}

fn report_subject(args: &ReportRenderArgs<'_>) -> CargoAllowResult<CoreSourceSubjectV1> {
    let semantic_identity = canonical_report_identity(args)?;
    let (kind, portable_identity, limitations) = match args.inventory_source_identity {
        Some(identity) => (
            CoreSourceSubjectKindV1::Index,
            identity.to_string(),
            Vec::new(),
        ),
        None => (
            CoreSourceSubjectKindV1::Worktree,
            format!(
                "worktree:{}:current-unpinned",
                args.inventory_facts.source.as_str()
            ),
            vec![
                "the current worktree result is not bound to a commit, tree, or Git-index identity"
                    .to_string(),
            ],
        ),
    };
    Ok(CoreSourceSubjectV1 {
        kind,
        repository_identity: format!("local-repository:{semantic_identity}"),
        portable_identity,
        base: None,
        head: None,
        paths: Vec::new(),
        limitations,
    })
}

fn canonical_report_identity(args: &ReportRenderArgs<'_>) -> CargoAllowResult<String> {
    canonical_semantic_identity(&render_detail_json(args), None)
}

/// Derive a relocation-stable semantic identity from a command's own JSON
/// artifact.
///
/// Absolute roots, config paths, and run-scoped fields are scrubbed first so
/// the same repository content yields the same identity from any checkout
/// location or working directory.
///
/// `redact_root` additionally replaces the selected root wherever it is
/// embedded *inside* a string value — suggested commands, diagnostics, and
/// rendered paths — which key scrubbing alone cannot reach. Callers whose
/// artifacts already carry no root-derived prose pass `None` so their existing
/// identities are unaffected.
pub(crate) fn canonical_semantic_identity(
    artifact_json: &str,
    redact_root: Option<&Path>,
) -> CargoAllowResult<String> {
    let mut value: Value = serde_json::from_str(artifact_json).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to parse report identity input: {error}"),
        )
    })?;
    scrub_non_semantic_fields(&mut value);
    if let Some(root) = redact_root {
        redact_root_text(&mut value, root);
    }
    sort_json_keys(&mut value);
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to canonicalize report identity input: {error}"),
        )
    })?;
    Ok(allow_core::sha256_v1_bytes(&bytes))
}

fn scrub_non_semantic_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in [
                "root",
                "source_tree_root",
                "policy_config",
                "started_at",
                "run_id",
            ] {
                map.remove(key);
            }
            for child in map.values_mut() {
                scrub_non_semantic_fields(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                scrub_non_semantic_fields(child);
            }
        }
        _ => {}
    }
}

/// Replace every embedded occurrence of the selected root with a stable
/// placeholder, in both native and forward-slash spelling, so a relocated or
/// differently-spelled checkout of the same content hashes identically.
fn redact_root_text(value: &mut Value, root: &Path) {
    let spellings = root_spellings(root);
    if spellings.is_empty() {
        return;
    }
    redact_root_spellings(value, &spellings);
}

/// Every spelling of the root that can appear inside an artifact string.
///
/// A Windows artifact can carry the native backslash form, the portable
/// forward-slash form, and — because `#3180` strips the Win32 verbatim prefix
/// from operator-facing text while the resolved root may still carry it — the
/// prefix-stripped form of either. Longest first, so a longer spelling is never
/// left half-redacted by a shorter prefix of itself.
fn root_spellings(root: &Path) -> Vec<String> {
    let native = root.to_string_lossy().to_string();
    // `strip_win32_verbatim_prefix` returns the forward-slash form, so each
    // base is expanded into both separator spellings rather than assuming one.
    let bases = [
        native.clone(),
        allow_core::strip_win32_verbatim_prefix(&native),
    ];
    let mut spellings = Vec::new();
    for base in bases {
        spellings.push(base.replace('\\', "/"));
        spellings.push(base.replace('/', "\\"));
        spellings.push(base);
    }
    // A root that is only separators (`/` or `\`) would match every path
    // separator in the document and destroy its structure. Such a root is not a
    // real repository checkout, so redact nothing.
    spellings.retain(|spelling| {
        spelling
            .chars()
            .any(|character| !matches!(character, '/' | '\\'))
    });
    // Longest first, so a longer spelling is never left half-redacted by a
    // shorter prefix of itself.
    spellings.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    spellings.dedup();
    spellings
}

fn redact_root_spellings(value: &mut Value, spellings: &[String]) {
    const PLACEHOLDER: &str = "<repository-root>";
    match value {
        Value::Object(map) => {
            for child in map.values_mut() {
                redact_root_spellings(child, spellings);
            }
        }
        Value::Array(values) => {
            for child in values {
                redact_root_spellings(child, spellings);
            }
        }
        Value::String(text) => {
            for spelling in spellings {
                if text.contains(spelling.as_str()) {
                    *text = text.replace(spelling.as_str(), PLACEHOLDER);
                }
            }
        }
        _ => {}
    }
}

fn sort_json_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, mut child) in entries {
                sort_json_keys(&mut child);
                map.insert(key, child);
            }
        }
        Value::Array(values) => {
            for child in values {
                sort_json_keys(child);
            }
        }
        _ => {}
    }
}

/// Map already-computed inventory facts onto summary coverage.
///
/// Every command that projects the common summary reads coverage from the same
/// facts its own report already carries, so `audit`, `check`, `explain`, `why`,
/// and `worklist` cannot disagree about whether a run was complete.
pub(crate) fn summary_completeness(facts: &crate::InventoryFacts) -> CompletenessV1 {
    if facts.rust_files_skipped > 0
        || facts.rust_files_with_parse_errors > 0
        || facts.deleted_tracked.unwrap_or(0) > 0
        || facts.empty_git_tracked
    {
        return CompletenessV1::Partial;
    }
    // `allow_inventory::inventory` assigns Partial before Scoped when deleted,
    // submodule, or skipped paths exist. The explicit Rust scanner checks above
    // cover later read/parse omissions.
    match facts.completeness {
        InventoryCompleteness::Complete | InventoryCompleteness::Scoped => CompletenessV1::Complete,
        InventoryCompleteness::Fallback | InventoryCompleteness::Partial => CompletenessV1::Partial,
    }
}

fn report_advisory_count(args: &ReportRenderArgs<'_>) -> usize {
    let outcomes = args
        .outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .count();
    outcomes
        + args.evidence.policy_missing_evidence_entries
        + args.evidence.broken_evidence_links
        + args.evidence.weak_evidence_references
        + args.evidence.occurrence_headroom_entries
}

/// Explain, in the operator's own vocabulary, why coverage was not complete.
pub(crate) fn partial_coverage_reason(facts: &crate::InventoryFacts) -> String {
    let mut reasons = Vec::new();
    if facts.empty_git_tracked {
        reasons.push("Git reported no tracked files".to_string());
    }
    if facts.deleted_tracked.unwrap_or(0) > 0 {
        reasons.push(format!(
            "{} tracked path(s) are absent from the worktree",
            facts.deleted_tracked.unwrap_or(0)
        ));
    }
    if facts.rust_files_skipped > 0 {
        reasons.push(format!(
            "{} Rust file(s) were skipped",
            facts.rust_files_skipped
        ));
    }
    if facts.rust_files_with_parse_errors > 0 {
        reasons.push(format!(
            "{} Rust file(s) contained parse errors",
            facts.rust_files_with_parse_errors
        ));
    }
    if matches!(
        facts.completeness,
        InventoryCompleteness::Fallback | InventoryCompleteness::Partial
    ) {
        reasons.push(format!(
            "inventory completeness is {}",
            facts.completeness.as_str()
        ));
    }
    if reasons.is_empty() {
        "source inventory or scanner coverage is incomplete".to_string()
    } else {
        reasons.join("; ")
    }
}

fn render_detail(args: &ReportRenderArgs<'_>) -> String {
    let source_context = SourceTreeReportContext::new_with_identity(
        args.root,
        args.inventory_facts,
        args.inventory_source_identity,
    );
    let mut context = source_context.report(Some(args.baseline_debt_entries));
    args.evidence.apply_to(&mut context);
    context.enforcement = args.enforcement;
    context.style = if args.format == OutputFormat::Human && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    match args.format {
        OutputFormat::Human => allow_report::render_human_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Json => allow_report::render_json_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
    }
}

fn render_detail_json(args: &ReportRenderArgs<'_>) -> String {
    let source_context = SourceTreeReportContext::new_with_identity(
        args.root,
        args.inventory_facts,
        args.inventory_source_identity,
    );
    let mut context = source_context.report(Some(args.baseline_debt_entries));
    args.evidence.apply_to(&mut context);
    context.enforcement = args.enforcement;
    allow_report::render_json_with_context(
        args.command,
        args.findings,
        args.outcomes,
        args.failed,
        context,
    )
}

fn validate_summary_output_path(
    root: &Path,
    config: &SummaryOutputConfig,
) -> CargoAllowResult<PathBuf> {
    let path = resolve_under_root(root, &config.path);
    crate::assert_path_within_root(root, &path)?;
    // Resolve each candidate against the base its own command writes with, so a
    // relative `--root` can neither hide a real collision nor invent one between
    // two paths that land on different files.
    let cwd = std::env::current_dir().ok();
    for (base, conflict) in &config.conflicts {
        let candidate = match base {
            ConflictBase::SourceTreeRoot => Some(resolve_under_root(root, conflict)),
            ConflictBase::WorkingDirectory => {
                cwd.as_deref().map(|cwd| resolve_under_root(cwd, conflict))
            }
        };
        if candidate.is_some_and(|candidate| same_path(&path, &candidate)) {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--command-summary-output must differ from --output, --receipt, and --config",
            ));
        }
    }
    reject_tracked_summary_output(root, &path)?;
    Ok(path)
}

fn reject_tracked_summary_output(root: &Path, output: &Path) -> CargoAllowResult<()> {
    if !allow_inventory::git_worktree_metadata_present(root) {
        return Ok(());
    }
    let tracked = allow_inventory::git_ls_files(root).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Inventory,
            format!("cannot verify tracked summary-output collision: {error}"),
        )
    })?;
    let relative = output
        .strip_prefix(root)
        .map(allow_core::normalize_path)
        .unwrap_or_default();
    if tracked
        .iter()
        .any(|path| allow_core::normalize_path(path) == relative)
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--command-summary-output may not overwrite a tracked or staged repository file",
        ));
    }
    Ok(())
}

fn resolve_under_root(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn comparable_path(path: &Path) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    let Some(file_name) = path.file_name() else {
        return path.to_path_buf();
    };
    path.parent()
        .and_then(|parent| parent.canonicalize().ok())
        .map(|parent| parent.join(file_name))
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
mod tests;
