use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, MatchOutcome,
};
use allow_match::{CheckMode, evaluate, score_match};
use std::path::Path;

use crate::{SourceTreeReportContext, load_world_with_evidence_validation, write_file};

#[path = "explain_args.rs"]
mod explain_args;
#[path = "explain_render.rs"]
mod explain_render;
#[path = "explain_types.rs"]
mod explain_types;
pub(crate) use explain_args::ExplainArgs;
use explain_args::ExplainFormat;
use explain_render::{render_explain_entry, render_explain_entry_json};
pub(super) use explain_types::ExplainContext;

pub(crate) fn cmd_explain(args: &ExplainArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_validation(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        false,
    )?;
    let entry = cfg
        .allow
        .iter()
        .find(|e| e.id == args.id)
        .ok_or_else(|| CargoAllowError::new(format!("no allow entry `{}`", args.id)))?;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ExplainContext {
        inventory: source_context.inventory(),
    };
    let text = match args.format {
        ExplainFormat::Human => explain_entry_text(&root, &cfg, entry, &findings),
        ExplainFormat::Json => explain_entry_json(&root, &cfg, entry, &findings, context),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

fn explain_entry_text(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry(root, entry, &matching_findings, &outcomes)
}

fn explain_entry_json(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    context: ExplainContext<'_>,
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry_json(root, entry, &matching_findings, &outcomes, context)
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
    };
    explain_entry_json(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
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
