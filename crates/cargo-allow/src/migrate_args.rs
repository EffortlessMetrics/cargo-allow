use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct MigrateArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Legacy, bespoke xtask/ripr (`dialect = "xtask-ripr"`), or canonical policy file to migrate.
    #[arg(long)]
    pub(super) from: Option<PathBuf>,
    /// Directory containing compatible legacy policy files.
    #[arg(long)]
    pub(super) repo_policy: Option<PathBuf>,
    /// Output canonical policy path.
    #[arg(
        long = "output",
        visible_alias = "out",
        default_value = "policy/allow.toml"
    )]
    pub(super) out: PathBuf,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    pub(super) force: bool,
    /// Update the output policy in place via atomic replace instead of
    /// requiring --force when the target already exists. The output is
    /// validated before writing and unrelated entries are preserved.
    /// Mutually exclusive with --force.
    #[arg(long)]
    pub(super) update: bool,
    /// Summary output format. JSON requires --summary-output so it cannot be
    /// mixed with policy or warning text on stderr. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) summary_format: HumanJsonFormat,
    /// Write migration summary to a file. Required with --summary-format json.
    #[arg(long)]
    pub(super) summary_output: Option<PathBuf>,
}
