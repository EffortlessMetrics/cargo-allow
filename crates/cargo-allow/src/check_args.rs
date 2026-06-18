use clap::Parser;
use std::path::PathBuf;

use crate::{OutputFormat, ProfileArg, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct CheckArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path. With --profile spec-system, profile config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Opt-in profile to run instead of the default source-exception ledger.
    #[arg(long, value_enum)]
    pub(crate) profile: Option<ProfileArg>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    pub(crate) compat: bool,
    /// Filter findings by kind.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(crate) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(crate) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Write machine-readable receipt to a file.
    #[arg(long)]
    pub(crate) receipt: Option<PathBuf>,
    /// Check mode. Defaults to the policy-configured source-tree gate mode.
    #[arg(long, value_parser = ["audit", "no-new", "strict", "release"])]
    pub(crate) mode: Option<String>,
    /// Promote one receipt `advisory` count class to a blocking failure.
    /// Repeatable. `occurrence_headroom` is not available yet (#1472).
    #[arg(long = "deny", value_name = "STATUS")]
    pub(crate) deny: Vec<String>,
}
