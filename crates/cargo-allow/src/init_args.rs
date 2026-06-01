use clap::Parser;
use std::path::PathBuf;

use crate::RootArgs;

#[derive(Debug, Clone, Parser)]
pub(crate) struct InitArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Write strict-mode defaults.
    #[arg(long)]
    pub(crate) strict: bool,
    /// Overwrite an existing policy file.
    #[arg(long)]
    pub(crate) force: bool,
    /// Policy config path.
    #[arg(long, default_value = "policy/allow.toml")]
    pub(crate) config: PathBuf,
}
