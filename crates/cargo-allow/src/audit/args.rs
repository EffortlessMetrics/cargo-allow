use clap::Parser;
use std::path::PathBuf;

use crate::{OutputFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReportArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    pub(crate) compat: bool,
    /// Filter findings by kind.
    #[arg(long)]
    pub(crate) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(crate) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}
