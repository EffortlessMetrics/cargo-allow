use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{ProfileArg, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct DoctorArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Optional governance profile to diagnose instead of the source-exception setup.
    #[arg(long, value_enum)]
    pub(super) profile: Option<ProfileArg>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = DoctorFormat::Human)]
    pub(super) format: DoctorFormat,
    /// Write doctor output to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Exit non-zero if the policy is invalid or evidence is broken.
    #[arg(long)]
    pub(super) require_clean: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum DoctorFormat {
    Human,
    Json,
}
