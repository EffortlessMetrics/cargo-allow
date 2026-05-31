use clap::Parser;
use std::path::PathBuf;

use crate::{OutputFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct DiffArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Filter source findings and allow-entry policy changes by kind.
    #[arg(long)]
    pub(super) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(super) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Base git revision for changed-file listing.
    #[arg(long)]
    pub(super) base: String,
    /// Optional head git revision.
    #[arg(long)]
    pub(super) head: Option<String>,
}
