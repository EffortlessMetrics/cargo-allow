use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct RefreshArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Allow entry id with advisory location drift to refresh.
    /// Required unless --all is given (#3184).
    #[arg(long, required_unless_present = "all")]
    pub(crate) allow_id: Option<String>,
    /// Refresh all location-drifted entries in one pass (#3184).
    #[arg(long, conflicts_with = "allow_id")]
    pub(crate) all: bool,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    pub(crate) dry_run: bool,
    /// Update last_seen in the policy file after operator review.
    #[arg(long, conflicts_with = "dry_run")]
    pub(crate) write: bool,
    /// Include untracked files when scanning current findings.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(crate) format: HumanJsonFormat,
    /// Write refresh preview/result to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}
