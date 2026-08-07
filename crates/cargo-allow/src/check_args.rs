use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{OutputFormat, ProfileArg, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct CheckArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path. With --profile spec-system, profile config path.
    /// Can also be set via the CARGO_ALLOW_CONFIG environment variable (#3230).
    #[arg(long, env = "CARGO_ALLOW_CONFIG")]
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
    /// Check mode [possible values: no-new, audit, strict, release].
    ///   no-new   Fail on new/expired/ambiguous/invalid_selector/
    ///            missing_required_field/evidence_missing (CI gate)
    ///   audit    Report all advisory statuses without failing (informational)
    ///   strict   Fail on any non-matched status except location_drift
    ///            (so stale/review_due/baseline_debt and the no-new set fail)
    ///   release  Currently equivalent to strict; advisory escalation is
    ///            driven by --deny, not by this mode
    /// Can also be set via the CARGO_ALLOW_MODE environment variable (#3230).
    #[arg(long, value_parser = ["audit", "no-new", "strict", "release"], env = "CARGO_ALLOW_MODE")]
    pub(crate) mode: Option<String>,
    /// Promote one receipt `advisory` count class to a blocking failure.
    /// Repeatable. Supported classes mirror receipt `advisory` fields, including `occurrence_headroom`.
    #[arg(long = "deny", value_name = "STATUS")]
    pub(crate) deny: Vec<String>,
    /// Evaluation phase for profile-specific checks.
    #[arg(long, value_enum)]
    pub(crate) phase: Option<CheckPhase>,
    /// Evaluate the exact Git index candidate instead of the worktree.
    #[arg(long)]
    pub(crate) staged: bool,
    /// Print only the canonical staged candidate identity.
    #[arg(long)]
    pub(crate) staged_identity_only: bool,
    /// Require this staged identity before evaluating or publishing a result.
    #[arg(long)]
    pub(crate) expect_staged_identity: Option<String>,
    /// Tool selection mode for self-hosted staged evaluation.
    #[arg(long, value_enum)]
    pub(crate) tool_mode: Option<crate::precommit_tool::ToolSelectionMode>,
    /// Expected digest for the selected prebuilt cargo-allow executable.
    #[arg(long)]
    pub(crate) tool_digest: Option<String>,
    /// Authorize source-preview evidence for an explicit tool-under-test.
    #[arg(long)]
    pub(crate) preview_authorized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CheckPhase {
    Precommit,
}
