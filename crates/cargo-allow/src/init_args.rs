use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub(crate) struct InitArgs {
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
