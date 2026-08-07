use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Preview stale allow entries.
    #[arg(long)]
    pub(super) stale: bool,
    /// Target a single entry by allow ID instead of all stale entries (#3184).
    #[arg(long, value_name = "ALLOW_ID")]
    pub(super) allow_id: Option<String>,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    pub(super) dry_run: bool,
    /// Remove stale entries from the policy file.
    #[arg(long, conflicts_with = "dry_run")]
    pub(super) write: bool,
    /// Include untracked files when determining stale entries.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) format: HumanJsonFormat,
    /// Write prune preview/result to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
}
