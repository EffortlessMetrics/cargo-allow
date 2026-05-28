use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::RootArgs;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ProposeArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    pub(super) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Expiry date for generated baseline_debt entries. Defaults to 67 days from today.
    #[arg(long)]
    pub(super) expires: Option<String>,
    /// Write proposed policy to this path.
    #[arg(long)]
    pub(super) write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    pub(super) force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = ProposeSummaryFormat::Human)]
    pub(super) summary_format: ProposeSummaryFormat,
    /// Write proposal summary to a file instead of stderr.
    #[arg(long)]
    pub(super) summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum ProposeSummaryFormat {
    Human,
    Json,
}
