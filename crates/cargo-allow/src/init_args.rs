use clap::Parser;
use std::path::PathBuf;

use crate::{ProfileArg, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct InitArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Write strict-mode defaults.
    #[arg(long)]
    pub(crate) strict: bool,
    /// Optional governance profile to bootstrap instead of the source-exception policy.
    #[arg(long, value_enum)]
    pub(crate) profile: Option<ProfileArg>,
    /// Show files that would be created without writing them.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Overwrite an existing policy file.
    #[arg(long)]
    pub(crate) force: bool,
    /// Policy config path.
    #[arg(long, default_value = "policy/allow.toml")]
    pub(crate) config: PathBuf,
}
