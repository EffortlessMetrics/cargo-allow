use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, ProfileArg, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct DoctorArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Optional governance profile to diagnose instead of the source-exception setup.
    #[arg(long, value_enum)]
    pub(crate) profile: Option<ProfileArg>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(crate) format: HumanJsonFormat,
    /// Write doctor output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Write a redacted, read-only support bundle to a source-tree path.
    #[arg(long)]
    pub(crate) support_bundle: Option<PathBuf>,
    /// Exit non-zero if the policy is invalid or evidence is broken.
    #[arg(long)]
    pub(crate) require_clean: bool,
}
