use allow_core::{CargoAllowError, CargoAllowResult, Finding, LastSeen};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};

#[path = "refresh_args.rs"]
mod refresh_args;
#[path = "refresh_render.rs"]
mod refresh_render;
#[path = "refresh_select.rs"]
mod refresh_select;
#[path = "refresh_types.rs"]
mod refresh_types;
pub(crate) use refresh_args::{RefreshArgs, RefreshFormat};
use refresh_render::{render_refresh_json, render_refresh_result};
use refresh_select::{apply_last_seen_refresh, select_location_drift_refresh};
use refresh_types::RefreshContext;

use crate::{
    EvidenceValidationMode, SourceTreeReportContext, config_path, emit_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    load_world_with_evidence_mode, write_file,
};

pub(crate) fn cmd_refresh(args: &RefreshArgs) -> CargoAllowResult<()> {
    if args.dry_run && args.write {
        return Err(CargoAllowError::new(
            "pass either --dry-run or --write, not both",
        ));
    }
    let (root, mut cfg, findings, inventory_facts) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let (entry_index, finding_index, drift_message) =
        select_location_drift_refresh(&cfg, &outcomes, &findings, &args.allow_id)?;
    let finding = &findings[finding_index];
    let previous_last_seen = cfg.allow[entry_index].last_seen.clone();
    let mut preview_entry = cfg.allow[entry_index].clone();
    apply_last_seen_refresh(&mut preview_entry, finding);
    let written_path = if args.write {
        apply_last_seen_refresh(&mut cfg.allow[entry_index], finding);
        validate_policy(&cfg)?;
        let evidence_source_tree_files =
            current_evidence_source_tree_files(&root, args.include_untracked);
        validate_evidence_references_for_source_tree(
            &root,
            &cfg,
            evidence_source_tree_files.as_ref(),
        )?;
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        write_file(&path, &render_policy(&cfg))?;
        Some(path)
    } else {
        None
    };
    let entry_for_render = if args.write {
        &cfg.allow[entry_index]
    } else {
        &preview_entry
    };
    render_and_emit(
        args,
        entry_for_render,
        finding,
        previous_last_seen,
        &drift_message,
        &root,
        inventory_facts,
        written_path.as_deref(),
    )
}

fn render_and_emit(
    args: &RefreshArgs,
    entry: &allow_core::AllowEntry,
    finding: &Finding,
    previous_last_seen: Option<LastSeen>,
    drift_message: &str,
    root: &std::path::Path,
    inventory_facts: crate::InventoryFacts,
    written_path: Option<&std::path::Path>,
) -> CargoAllowResult<()> {
    let source_context = SourceTreeReportContext::new(root, inventory_facts);
    let context = RefreshContext {
        inventory: source_context.inventory(),
    };
    let written = written_path.map(|path| path.display().to_string());
    let written_ref = written.as_deref();
    let text = match args.format {
        RefreshFormat::Human => render_refresh_result(
            entry,
            finding,
            previous_last_seen,
            drift_message,
            args.dry_run,
            args.write,
            written_ref,
            context,
        ),
        RefreshFormat::Json => render_refresh_json(
            entry,
            finding,
            previous_last_seen,
            drift_message,
            args.dry_run,
            args.write,
            written_ref,
            context,
        ),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_refresh_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Selector, Span, StructuralIdentity};
    let entry = allow_core::AllowEntry {
        id: "fixture-refresh-drift".to_string(),
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "lint".to_string(),
        classification: "reviewed_lint_exception".to_string(),
        reason: "Fixture refresh receipt".to_string(),
        evidence: vec!["test:refresh-receipt-fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle {
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: None,
        },
        selector: Selector::default(),
        last_seen: Some(LastSeen { line: 22, column: 4 }),
    };
    let finding = Finding {
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: "src/lib.rs".into(),
        identity: StructuralIdentity::new("rust", "attribute"),
        message: "fixture refresh drift".to_string(),
        span: Some(Span {
            line: 22,
            column: 4,
        }),
    };
    render_refresh_json(
        &entry,
        &finding,
        Some(LastSeen { line: 14, column: 8 }),
        "allow-drift last_seen changed from 14:8 to 22:4",
        true,
        false,
        None,
        RefreshContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "filesystem_include_untracked",
                Some("tests/fixtures/refresh/advisory-drift"),
                Some(2),
            ),
        },
    )
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
