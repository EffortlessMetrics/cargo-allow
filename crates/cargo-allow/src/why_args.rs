use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct WhyArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Finding kind near the location (required to disambiguate).
    /// Same vocabulary as check/audit/diff: panic, unsafe, lint-exception,
    /// non-rust, generated, policy-exception.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(super) kind: String,
    /// Path containing the finding.
    #[arg(long)]
    pub(super) path: PathBuf,
    /// Line near the finding.
    #[arg(long)]
    pub(super) line: u32,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = WhyFormat::Human)]
    pub(super) format: WhyFormat,
    /// Write explanation output to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Write a versioned, non-mutating add-finding plan artifact.
    #[arg(long)]
    pub(super) plan: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum WhyFormat {
    Human,
    Json,
}
