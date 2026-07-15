use clap::Parser;
use std::path::PathBuf;

use crate::RootArgs;

#[derive(Debug, Clone, Parser)]
pub(crate) struct WhyArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Finding kind near the location (required to disambiguate).
    #[arg(long)]
    pub(super) kind: String,
    /// Path containing the finding.
    #[arg(long)]
    pub(super) path: PathBuf,
    /// Line near the finding.
    #[arg(long)]
    pub(super) line: u32,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Write explanation output to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
}
