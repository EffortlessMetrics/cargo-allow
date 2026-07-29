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
    let entry = cfg.allow.iter().find(|e| e.id == args.id).ok_or_else(|| {
        CargoAllowError::new(format!(
            "no allow entry `{}`; run `cargo-allow list` to see valid IDs",
            args.id
        ))
    })?;
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
    let text = match args.format {
        HumanJsonFormat::Human => explain_entry_text_with_source_tree_files(
            &root,
            &cfg,
            entry,
            &findings,
            evidence_source_tree_files.as_ref(),
            style,
        ),
        HumanJsonFormat::Json => explain_entry_json_with_source_tree_files(
            &root,
            &cfg,
            entry,
            &findings,
            evidence_source_tree_files.as_ref(),
            context,
        ),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
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
