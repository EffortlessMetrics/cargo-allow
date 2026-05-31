use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::RootArgs;

#[derive(Debug, Clone, Parser)]
pub(crate) struct AddArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Finding kind to add.
    #[arg(long)]
    pub(super) kind: String,
    /// Path containing the finding.
    #[arg(long)]
    pub(super) path: PathBuf,
    /// Line near the finding.
    #[arg(long)]
    pub(super) line: u32,
    /// Owner for the retained exception.
    #[arg(long)]
    pub(super) owner: String,
    /// Reason this exception is acceptable.
    #[arg(long)]
    pub(super) reason: String,
    /// Classification for the retained exception.
    #[arg(long, default_value = "reviewed_exception")]
    pub(super) classification: String,
    /// Review date for the retained exception. Defaults to roughly 90 days from today.
    #[arg(long)]
    pub(super) review_after: Option<String>,
    /// Optional expiry date for the retained exception.
    #[arg(long)]
    pub(super) expires: Option<String>,
    /// Evidence reference supporting this exception.
    #[arg(long)]
    pub(super) evidence: Vec<String>,
    /// Entry ID. Defaults to the next allow-NNNN ID.
    #[arg(long)]
    pub(super) id: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Write proposed policy to this path.
    #[arg(long)]
    pub(super) write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    pub(super) force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = AddSummaryFormat::Human)]
    pub(super) summary_format: AddSummaryFormat,
    /// Write add summary to a file instead of stderr.
    #[arg(long)]
    pub(super) summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum AddSummaryFormat {
    Human,
    Json,
}
