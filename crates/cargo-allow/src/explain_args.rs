use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, ProfileArg, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ExplainArgs {
    /// Allow entry ID.
    pub(super) id: String,
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path. With --profile spec-system, profile config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Opt-in profile to explain instead of the default source-exception ledger.
    #[arg(long, value_enum)]
    pub(crate) profile: Option<ProfileArg>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) format: HumanJsonFormat,
    /// Write explanation output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}
