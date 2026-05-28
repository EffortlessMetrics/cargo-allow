use allow_core::{CargoAllowError, CargoAllowResult, FindingKind};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, config_path, load_world, source_tree_root_text, write_file};

#[path = "prune_render.rs"]
mod prune_render;
#[path = "prune_stale.rs"]
mod prune_stale;
use prune_render::{render_prune_stale_json, render_prune_stale_result};
use prune_stale::{config_without_prune_candidates, prune_stale_candidates};

#[cfg(test)]
use allow_core::{AllowConfig, MatchOutcome, MatchStatus};

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
#[path = "prune_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "prune_tests.rs"]
mod tests;
