use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};
use allow_report::MutationReceipt;

use crate::{
    EvidenceValidationMode, HumanJsonFormat, MutationLock, SourceTreeReportContext, config_path,
    emit_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    git_relative_config_path, load_world_with_evidence_mode, resolve_source_tree_root,
};
use repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};

#[path = "prune_args.rs"]
mod prune_args;
#[path = "prune_render.rs"]
mod prune_render;
#[path = "prune_stale.rs"]
mod prune_stale;
#[path = "prune_types.rs"]
mod prune_types;
pub(crate) use prune_args::PruneArgs;
use prune_render::{render_prune_stale_json, render_prune_stale_result};
use prune_stale::{
    config_without_prune_candidates, prune_stale_candidates,
    removed_toml_blocks as stale_removed_toml_blocks,
};
use prune_types::{PruneCandidate, PruneContext, PruneRenderMode};

#[cfg(test)]
use crate::RootArgs;
#[cfg(test)]
use allow_core::{AllowConfig, FindingKind, MatchOutcome, MatchStatus};
#[cfg(test)]
use std::path::PathBuf;

pub(crate) fn cmd_prune(args: &PruneArgs) -> CargoAllowResult<()> {
    if !args.stale {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "prune currently supports only --stale",
        ));
    }
    if args.dry_run && args.write {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "pass either --dry-run or --write, not both",
        ));
    }
    let mutation_lock = if args.write {
        let cwd = std::env::current_dir()
            .map_err(|error| CargoAllowError::new(format!("failed to read cwd: {error}")))?;
        let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        crate::policy_config::assert_path_within_root(&root, &path)?;
        Some(MutationLock::acquire(path)?)
    } else {
        None
    };
    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let _mutation_lock = mutation_lock;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let candidates = prune_stale_candidates(&cfg, &outcomes);
    let policy_path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
    })?;
    let mut receipt_candidates = candidates.iter().collect::<Vec<_>>();
    receipt_candidates.sort_by(|left, right| left.id.cmp(&right.id));
    let before_fingerprints = receipt_candidates
        .iter()
        .map(|candidate| {
            cfg.allow
                .iter()
                .find(|entry| entry.id == candidate.id)
                .ok_or_else(|| {
                    CargoAllowError::new(format!(
                        "internal error: stale candidate {} is missing from policy",
                        candidate.id
                    ))
                })
                .map(|entry| Some(allow_core::allow_entry_content_fingerprint(entry)))
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;
    let repo_root = root.display().to_string();
    let config_source = crate::policy_config::git_relative_config_path(&root, Some(&policy_path))?
        .to_string_lossy()
        .replace('\\', "/");
    let recovery_command = format!("git diff -- {config_source}");
    let mutation_receipt = MutationReceipt {
        operation: "prune",
        tool_version: env!("CARGO_PKG_VERSION"),
        repo_root: Some(&repo_root),
        config_source: Some(&config_source),
        ledger_ids: Vec::new(),
        changed_allow_ids: receipt_candidates
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect(),
        before_fingerprints,
        after_fingerprints: vec![None; receipt_candidates.len()],
        result: if args.write && !candidates.is_empty() {
            "written"
        } else {
            "stdout"
        },
        next_commands: vec![
            recovery_command,
            "cargo-allow check --mode no-new".to_string(),
        ],
    };
    let rendered_policy = (args.format == HumanJsonFormat::Human && !candidates.is_empty())
        .then(|| render_policy(&cfg));
    let removed_toml_blocks = rendered_policy
        .as_deref()
        .map(|rendered| stale_removed_toml_blocks(rendered, &candidates))
        .unwrap_or_default();
    let written_path = if args.write && !candidates.is_empty() {
        let pruned = config_without_prune_candidates(&cfg, &candidates);
        validate_policy(&pruned)?;
        let evidence_source_tree_files =
            current_evidence_source_tree_files(&root, args.include_untracked);
        validate_evidence_references_for_source_tree(
            &root,
            &pruned,
            evidence_source_tree_files.as_ref(),
        )?;
        let policy_target = git_relative_config_path(&root, args.config.as_deref())?;
        let rendered = render_policy(&pruned);
        apply_single_target(SingleTargetApplyRequest {
            repository_root: &root,
            target: &policy_target,
            contents: &rendered,
            caller_reference: Some("cargo-allow:prune"),
            lock_identity: Some(
                policy_target
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            ),
            mode: SingleTargetApplyMode::AtomicReplace,
        })
        .into_result()?;
        Some(policy_path.clone())
    } else {
        None
    };
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = PruneContext {
        inventory: source_context.inventory(),
        mutation_receipt,
    };
    let text = match args.format {
        HumanJsonFormat::Human => render_prune_stale_result(
            &candidates,
            &removed_toml_blocks,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            cfg.allow.len(),
            context,
        ),
        HumanJsonFormat::Json => render_prune_stale_json(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            cfg.allow.len(),
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
        0,
        PruneContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(49),
            ),
            mutation_receipt: allow_report::MutationReceipt {
                operation: "prune",
                tool_version: env!("CARGO_PKG_VERSION"),
                repo_root: Some("H:/Code/Rust/cargo-allow"),
                config_source: Some("policy/allow.toml"),
                ledger_ids: Vec::new(),
                changed_allow_ids: Vec::new(),
                before_fingerprints: Vec::new(),
                after_fingerprints: Vec::new(),
                result: "stdout",
                next_commands: vec!["cargo-allow check --mode no-new".to_string()],
            },
        },
    )
}

#[cfg(test)]
#[path = "prune_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "prune_tests.rs"]
mod tests;
