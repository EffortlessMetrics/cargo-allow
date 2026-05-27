use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowResult, FindingKind, MatchOutcome, MatchStatus,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{RootArgs, config_path, load_world, markdown_cell, source_tree_root_text, write_file};

#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Preview stale allow entries.
    #[arg(long)]
    stale: bool,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    dry_run: bool,
    /// Remove stale entries from the policy file.
    #[arg(long, conflicts_with = "dry_run")]
    write: bool,
    /// Include untracked files when determining stale entries.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = PruneFormat::Human)]
    format: PruneFormat,
    /// Write prune preview/result to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PruneFormat {
    Human,
    Json,
}

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
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let candidates = prune_stale_candidates(&cfg, &outcomes);
    let written_path = if args.write && !candidates.is_empty() {
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        let pruned = config_without_prune_candidates(&cfg, &candidates);
        validate_policy(&pruned)?;
        write_file(&path, &render_policy(&pruned))?;
        Some(path)
    } else {
        None
    };
    let root_text = source_tree_root_text(&root);
    let context = PruneContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
    };
    let text = match args.format {
        PruneFormat::Human => render_prune_stale_result(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
        ),
        PruneFormat::Json => render_prune_stale_json(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            context,
        ),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PruneContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
}

impl Default for PruneContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PruneCandidate {
    id: String,
    kind: FindingKind,
    family: Option<String>,
    owner: String,
    classification: String,
    scope: String,
    reason: String,
}

fn prune_stale_candidates(cfg: &AllowConfig, outcomes: &[MatchOutcome]) -> Vec<PruneCandidate> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Stale)
        .filter_map(|outcome| {
            let id = outcome.allow_id.as_deref()?;
            let entry = cfg.allow.iter().find(|entry| entry.id == id)?;
            Some(PruneCandidate {
                id: entry.id.clone(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                reason: entry.reason.clone(),
            })
        })
        .collect()
}

fn config_without_prune_candidates(
    cfg: &AllowConfig,
    candidates: &[PruneCandidate],
) -> AllowConfig {
    let mut pruned = cfg.clone();
    pruned
        .allow
        .retain(|entry| !candidates.iter().any(|candidate| candidate.id == entry.id));
    pruned
}

fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow prune\n\n");
    if write_requested {
        out.push_str("mode: write\n");
    } else {
        out.push_str("mode: dry-run\n");
    }
    if explicit_dry_run {
        out.push_str("requested: --dry-run\n");
    }
    out.push_str(&format!("stale entries: {}\n\n", candidates.len()));
    if candidates.is_empty() {
        out.push_str("No stale allow entries found.\n");
        return out;
    }
    out.push_str("| Allow ID | Kind | Family | Owner | Classification | Scope | Reason |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for candidate in candidates {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&candidate.id),
            candidate.kind,
            markdown_cell(candidate.family.as_deref().unwrap_or("-")),
            markdown_cell(&candidate.owner),
            markdown_cell(&candidate.classification),
            markdown_cell(&candidate.scope),
            markdown_cell(&candidate.reason)
        ));
    }
    if let Some(path) = written_path {
        out.push_str(&format!(
            "\nRemoved stale entries from `{}`.\n",
            markdown_cell(&path.display().to_string())
        ));
    } else {
        out.push_str(
            "\nNo files were changed. Remove these entries only after confirming the exception is gone.\n",
        );
    }
    out
}

fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    context: PruneContext<'_>,
) -> String {
    let written = written_path.map(|path| path.display().to_string());
    let report_candidates = candidates
        .iter()
        .map(|candidate| allow_report::PruneCandidate {
            id: &candidate.id,
            kind: candidate.kind.as_str(),
            family: candidate.family.as_deref(),
            owner: &candidate.owner,
            classification: &candidate.classification,
            scope: &candidate.scope,
            reason: &candidate.reason,
        })
        .collect::<Vec<_>>();
    allow_report::render_prune_json(
        &report_candidates,
        allow_report::PruneModeContext {
            explicit_dry_run,
            write_requested,
            written_path: written.as_deref(),
        },
        allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
    )
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
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(49),
        },
    )
}

#[cfg(test)]
#[path = "prune_tests.rs"]
mod tests;
