use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs, parse_kind_filter_arg};

/// Explain why a finding at a path/line is unreceipted.
///
/// Run `cargo-allow check --format json` (or `cargo-allow audit --format json`)
/// first to find the kind, path, and line of an unreceipted finding, then pass
/// those coordinates to `why`. Use `cargo-allow vocabulary` to list accepted
/// kind values.
#[derive(Debug, Clone, Parser)]
pub(crate) struct WhyArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
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
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) format: HumanJsonFormat,
    /// Write explanation output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Write a versioned, non-mutating add-finding plan artifact.
    #[arg(long)]
    pub(crate) plan: Option<PathBuf>,
}
