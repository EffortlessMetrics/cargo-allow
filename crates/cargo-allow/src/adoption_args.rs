use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct AdoptionArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Include files outside the Git-tracked inventory.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Request the strict empty-policy bootstrap preview for a clean repository.
    #[arg(long)]
    pub(crate) strict: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(crate) format: HumanJsonFormat,
    /// Write the adoption artifact to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}
