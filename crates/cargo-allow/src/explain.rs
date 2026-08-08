use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    MatchOutcome,
};
use allow_match::{CheckMode, evaluate, score_match};
use std::path::Path;

use crate::{
    EvidenceValidationMode, HumanJsonFormat, ProfileArg, SourceTreeReportContext, emit_text,
    evidence_inventory::current_evidence_source_tree_files, load_world_with_evidence_mode,
    spec_system,
};

#[path = "explain_args.rs"]
mod explain_args;
#[path = "explain_render.rs"]
mod explain_render;
#[path = "explain_steps.rs"]
mod explain_steps;
#[path = "explain_types.rs"]
mod explain_types;
pub(crate) use explain_args::ExplainArgs;
#[cfg(test)]
use explain_render::{render_explain_entry_json, render_explain_entry_styled};
pub(super) use explain_types::ExplainContext;

pub(crate) fn cmd_explain(args: &ExplainArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        if args.include_untracked {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--include-untracked is not supported with --profile spec-system; remove --include-untracked or drop --profile spec-system",
            ));
        }
        return spec_system::cmd_spec_system_explain(spec_system::SpecSystemExplainCommandArgs {
            artifact_id: &args.id,
            root: &args.root,
            config: args.config.as_deref(),
            format_json: matches!(args.format, HumanJsonFormat::Json),
            output: args.output.as_deref(),
        });
    }

    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    // Normalize numeric shorthand: "42" → "allow-0042", matching the ledger's
    // zero-padded id format (#3166).
    let normalized_id = normalize_allow_id_shorthand(&args.id);
    let entry = cfg
        .allow
        .iter()
        .find(|e| e.id == normalized_id)
        .ok_or_else(|| missing_allow_entry_error(&normalized_id))?;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ExplainContext {
        inventory: source_context.inventory(),
    };
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    let (matching_findings, outcomes) = explain_entry_state(&cfg, entry, &findings);
    // One report build serves both renderings and the summary projection, so
    // the evidence and traceability references are read exactly once.
    let (detail_json, detail_human, suggested_actions) = explain_render::render_explain_report(
        &root,
        entry,
        &matching_findings,
        &outcomes,
        evidence_source_tree_files.as_ref(),
        context,
        |report| {
            (
                allow_report::render_explain_json(report),
                matches!(args.format, HumanJsonFormat::Human)
                    .then(|| allow_report::render_explain_human_styled(report, style)),
                report.suggested_actions.to_vec(),
            )
        },
    );
    // Common operator grammar (#3149). The detailed explain artifact remains
    // authoritative; this projection is additive and derived from the same
    // in-memory entry state without rescanning source or reloading policy.
    let summary = explain_summary(
        &detail_json,
        &root,
        &source_context,
        ExplainEntryFacts {
            entry,
            attention_status: outcomes
                .iter()
                .find(|outcome| outcome.status != allow_core::MatchStatus::Matched)
                .map(|outcome| outcome.status),
            matching_finding_count: matching_findings.len(),
            suggested_actions,
            inventory_facts,
        },
    )?;
    crate::core_command_router::write_summary_artifact(&root, &summary)?;

    let text = match args.format {
        HumanJsonFormat::Human => {
            let mut rendered =
                crate::core_command_summary::render_core_command_summary_human(&summary);
            rendered.push('\n');
            rendered.push_str(detail_human.as_deref().unwrap_or_default());
            rendered
        }
        HumanJsonFormat::Json => detail_json,
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

/// Entry-scoped facts `explain` has already computed for the common summary.
struct ExplainEntryFacts<'a> {
    entry: &'a AllowEntry,
    /// Status of the first outcome for this entry that is not `Matched`.
    attention_status: Option<allow_core::MatchStatus>,
    matching_finding_count: usize,
    suggested_actions: Vec<String>,
    inventory_facts: crate::InventoryFacts,
}

/// Build the common operator summary from the entry state explain already holds.
///
/// The relocation-stable semantic identity comes from explain's own JSON
/// artifact, exactly as `audit`, `check`, and `doctor` derive theirs, so the
/// summary never rescans source or reloads policy to describe itself.
fn explain_summary(
    detail_json: &str,
    root: &Path,
    source_context: &SourceTreeReportContext,
    facts: ExplainEntryFacts<'_>,
) -> CargoAllowResult<crate::core_command_summary::CoreCommandSummaryV1> {
    let semantic_identity =
        crate::core_command_router::canonical_semantic_identity(detail_json, Some(root))?;
    let completeness = crate::core_command_router::summary_completeness(&facts.inventory_facts);
    let coverage_limitation = (completeness != effortless_repo_protocol::CompletenessV1::Complete)
        .then(|| crate::core_command_router::partial_coverage_reason(&facts.inventory_facts));
    let inventory_source = source_context.inventory_source();

    // The subject is the explained ledger entry, named in the shared
    // `<subject>:<inventory mode>:current-unpinned` grammar the worktree
    // commands use, with the entry's own scope carried in `paths`.
    let mut subject = crate::core_command_summary::CoreSourceSubjectV1 {
        kind: crate::core_command_summary::CoreSourceSubjectKindV1::ScopedPath,
        repository_identity: format!("local-repository:{semantic_identity}"),
        portable_identity: format!(
            "scoped:allow-entry:{}:{inventory_source}:current-unpinned",
            facts.entry.id
        ),
        base: None,
        head: None,
        paths: entry_scope_paths(facts.entry),
        limitations: Vec::new(),
    };
    subject.limitations.push(
        "the current worktree result is not bound to a commit, tree, or Git-index identity"
            .to_string(),
    );

    crate::core_command_summary::core_command_summary_from_explain(
        crate::core_command_summary::ExplainSummaryFactsV1 {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            subject,
            completeness,
            coverage_limitation,
            allow_id: facts.entry.id.clone(),
            attention_status: facts.attention_status,
            matching_finding_count: facts.matching_finding_count,
            suggested_actions: facts.suggested_actions,
            claim_boundary: effortless_repo_protocol::ClaimBoundaryV1::new(
                "cargo-allow explained one source-exception ledger entry against current source-tree syntax only",
            )
            .with_limitations(vec![
                "cargo metadata, rustc, Clippy, build scripts, proc macros, tests, and repository code were not invoked"
                    .to_string(),
                "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
                    .to_string(),
                "one healthy entry does not prove the repository passes the no-new gate".to_string(),
            ]),
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build core command summary: {error}"),
        )
    })
}

/// The entry's own declared scope, if it declares one.
fn entry_scope_paths(entry: &AllowEntry) -> Vec<String> {
    if let Some(path) = entry.path.as_deref() {
        return vec![allow_core::normalize_path(path)];
    }
    entry
        .glob
        .as_deref()
        .map(|glob| vec![glob.to_string()])
        .unwrap_or_default()
}

fn missing_allow_entry_error(id: &str) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!("no allow entry `{id}`; run `cargo-allow list` to see valid IDs"),
    )
}

/// Normalize an allow-entry id shorthand for lookup.
///
/// If `raw` is purely numeric, expands to `allow-{raw:04}` (zero-padded to 4
/// digits, matching the ledger's standard id format). Non-numeric ids pass
/// through unchanged. So "42" → "allow-0042", "allow-0272" → "allow-0272".
fn normalize_allow_id_shorthand(raw: &str) -> String {
    if raw.chars().all(|c| c.is_ascii_digit()) && !raw.is_empty() {
        let n: usize = raw.parse().unwrap_or(0);
        format!("allow-{n:04}")
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
fn explain_entry_text(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> String {
    explain_entry_text_with_source_tree_files(
        root,
        cfg,
        entry,
        findings,
        None,
        allow_report::Style::PLAIN,
    )
}

#[cfg(test)]
fn explain_entry_text_with_source_tree_files(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    evidence_source_tree_files: Option<&std::collections::BTreeSet<String>>,
    style: allow_report::Style,
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry_styled(
        root,
        entry,
        &matching_findings,
        &outcomes,
        evidence_source_tree_files,
        style,
    )
}

#[cfg(test)]
fn explain_entry_json(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    context: ExplainContext<'_>,
) -> String {
    explain_entry_json_with_source_tree_files(root, cfg, entry, findings, None, context)
}

#[cfg(test)]
fn explain_entry_json_with_source_tree_files(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    evidence_source_tree_files: Option<&std::collections::BTreeSet<String>>,
    context: ExplainContext<'_>,
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry_json(
        root,
        entry,
        &matching_findings,
        &outcomes,
        evidence_source_tree_files,
        context,
    )
}

fn explain_entry_state(
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> (Vec<Finding>, Vec<MatchOutcome>) {
    let matching_findings = findings
        .iter()
        .filter(|finding| score_match(entry, finding).is_some())
        .cloned()
        .collect::<Vec<_>>();
    let mut single_entry_cfg = cfg.clone();
    single_entry_cfg.allow = vec![entry.clone()];
    let outcomes = evaluate(&single_entry_cfg, &matching_findings, CheckMode::NoNew);
    (matching_findings, outcomes)
}

#[cfg(test)]
pub(crate) fn sample_explain_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    let mut cfg = AllowConfig::empty();
    let entry = AllowEntry {
        id: "allow-json".to_string(),
        kind: FindingKind::NonRustFile,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };
    cfg.allow.push(entry.clone());
    let finding = Finding {
        kind: FindingKind::NonRustFile,
        family: None,
        path: PathBuf::from("tracked.file"),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", "tracked_file"),
        message: "test finding".to_string(),
        ledger: None,
    };
    explain_entry_json_with_source_tree_files(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
        None,
        ExplainContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(47),
            ),
        },
    )
}

#[cfg(test)]
#[path = "explain_artifact_tests.rs"]
mod artifact_tests;
#[cfg(test)]
#[path = "explain_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "explain_tests.rs"]
mod tests;
