use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::RootArgs;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ExplainArgs {
    /// Allow entry ID.
    pub(super) id: String,
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ExplainFormat::Human)]
    pub(super) format: ExplainFormat,
    /// Write explanation output to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum ExplainFormat {
    Human,
    Json,
}
