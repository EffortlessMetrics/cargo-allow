use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Preview stale allow entries.
    #[arg(long)]
    pub(crate) stale: bool,
    /// Target a single entry by allow ID instead of all stale entries (#3184).
    #[arg(long, value_name = "ALLOW_ID")]
    pub(crate) allow_id: Option<String>,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    pub(crate) dry_run: bool,
    /// Remove stale entries from the policy file.
    #[arg(long, conflicts_with = "dry_run")]
    pub(crate) write: bool,
    /// Include untracked files when determining stale entries.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(crate) format: HumanJsonFormat,
    /// Write prune preview/result to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}
