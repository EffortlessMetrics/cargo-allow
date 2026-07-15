use clap::Parser;
use std::path::PathBuf;

use crate::{OutputFormat, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct DiffArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Filter source findings and allow-entry policy changes by kind.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(super) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(super) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Base git revision for policy, finding, and changed-file posture comparison.
    #[arg(long)]
    pub(super) base: String,
    /// Optional head git revision. Defaults to the current working tree.
    #[arg(long)]
    pub(super) head: Option<String>,
    /// Require a revision note for weakening policy edits. When set, the diff
    /// fails if any policy change with posture_delta `worsened` or
    /// `review_required` lacks a matching note in --revisions-dir (#1475/#2075).
    #[arg(long)]
    pub(super) require_change_note: bool,
    /// Directory containing revision-note TOML files. Defaults to
    /// `.allow/revisions/`. Each file is an append-only note keyed on
    /// `allow_id` + `change_kind` and, for retained entries, exact
    /// before/after content fingerprints.
    #[arg(long, default_value = ".allow/revisions")]
    pub(super) revisions_dir: PathBuf,
}
