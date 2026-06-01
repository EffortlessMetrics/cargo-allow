use allow_core::{CargoAllowError, CargoAllowResult};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};

use crate::{
    EvidenceValidationMode, SourceTreeReportContext, config_path, emit_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    load_world_with_evidence_mode, write_file,
};

#[path = "prune_args.rs"]
mod prune_args;
#[path = "prune_render.rs"]
mod prune_render;
#[path = "prune_stale.rs"]
mod prune_stale;
#[path = "prune_types.rs"]
mod prune_types;
pub(crate) use prune_args::PruneArgs;
use prune_args::PruneFormat;
use prune_render::{render_prune_stale_json, render_prune_stale_result};
use prune_stale::{config_without_prune_candidates, prune_stale_candidates};
use prune_types::{PruneCandidate, PruneContext, PruneRenderMode};

#[cfg(test)]
use crate::RootArgs;
#[cfg(test)]
use allow_core::{AllowConfig, FindingKind, MatchOutcome, MatchStatus};
#[cfg(test)]
use std::path::PathBuf;

pub(crate) fn cmd_prune(args: &PruneArgs) -> CargoAllowResult<()> {
    if !args.stale {
        return Err(CargoAllowError::new(
            "prune currently supports only --stale",
        ));
    }
    if args.dry_run && args.write {
        return Err(CargoAllowError::new(
            "pass either --dry-run or --write, not both",
        ));
    }
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let candidates = prune_stale_candidates(&cfg, &outcomes);
    let written_path = if args.write && !candidates.is_empty() {
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        let pruned = config_without_prune_candidates(&cfg, &candidates);
        validate_policy(&pruned)?;
        let evidence_source_tree_files =
            current_evidence_source_tree_files(&root, args.include_untracked);
        validate_evidence_references_for_source_tree(
            &root,
            &pruned,
            evidence_source_tree_files.as_ref(),
        )?;
        write_file(&path, &render_policy(&pruned))?;
        Some(path)
    } else {
        None
    };
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = PruneContext {
        inventory: source_context.inventory(),
    };
    let text = match args.format {
        PruneFormat::Human => render_prune_stale_result(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            context,
        ),
        PruneFormat::Json => render_prune_stale_json(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            context,
        ),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_prune_json_for_contract_test() -> String {
    let candidates = Vec::new();
    render_prune_stale_json(
        &candidates,
        true,
        false,
        None,
        PruneContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(49),
            ),
        },
    )
}

#[cfg(test)]
#[path = "prune_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "prune_tests.rs"]
mod tests;
