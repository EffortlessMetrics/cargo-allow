use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct AddArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Finding kind to add. Required for ordinary target selection; omitted with
    /// `--from-plan`, where the kind is taken from the plan.
    /// Same vocabulary as check/audit/diff: panic, unsafe, lint-exception,
    /// non-rust, generated, policy-exception.
    #[arg(
        long,
        required_unless_present = "from_plan",
        conflicts_with = "from_plan",
        value_parser = parse_kind_filter_arg
    )]
    pub(super) kind: Option<String>,
    /// Path containing the finding. Use with --line to receipt one specific
    /// occurrence. Mutually exclusive with --glob. Requires --line.
    #[arg(long, requires = "line", conflicts_with = "glob")]
    pub(super) path: Option<PathBuf>,
    /// Line near the finding. Use with --path to receipt one specific
    /// occurrence. Mutually exclusive with --glob. Requires --path.
    #[arg(long, requires = "path")]
    pub(super) line: Option<u32>,
    /// Glob scope for a broad baseline (e.g. `src/foo.rs`, `src/**/*.rs`).
    /// Instead of receipting one occurrence, this receipts every current
    /// in-scope finding and pins the count as `occurrence_limit`, so the N+1th
    /// in-scope occurrence fails `check --mode no-new` (#2056). Mutually
    /// exclusive with --path/--line.
    #[arg(long, conflicts_with = "path")]
    pub(super) glob: Option<String>,
    /// Family filter for a --glob baseline (e.g. `unwrap`, `expect`). Narrows
    /// which in-scope findings the broad selector matches. Only meaningful with
    /// --glob.
    #[arg(long)]
    pub(super) family: Option<String>,
    /// Callee filter for a --glob baseline (e.g. `unwrap`). Narrows which
    /// in-scope findings the broad selector matches. Only meaningful with
    /// --glob.
    #[arg(long)]
    pub(super) callee: Option<String>,
    /// Owner for the retained exception.
    #[arg(long)]
    pub(super) owner: String,
    /// Reason this exception is acceptable.
    #[arg(long)]
    pub(super) reason: String,
    /// Classification for the retained exception.
    #[arg(long, default_value = "reviewed_exception")]
    pub(super) classification: String,
    /// Review date for the retained exception. Defaults to roughly 90 days from today.
    #[arg(long)]
    pub(super) review_after: Option<String>,
    /// Optional expiry date for the retained exception.
    #[arg(long)]
    pub(super) expires: Option<String>,
    /// Evidence reference supporting this exception.
    #[arg(long)]
    pub(super) evidence: Vec<String>,
    /// Entry ID. Defaults to the next allow-NNNN ID.
    #[arg(long)]
    pub(super) id: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Write proposed policy to this path.
    #[arg(long)]
    pub(super) write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    pub(super) force: bool,
    /// Update the live policy in place instead of printing/writing a candidate
    /// file. Resolves the discovered policy/allow.toml, validates the full
    /// result, and atomically replaces it. Recommended for adding one receipt.
    /// Mutually exclusive with --write.
    #[arg(long, conflicts_with = "write")]
    pub(super) update: bool,
    /// Preview the entry that would be added without writing any file (#3189).
    /// Compatible with --write and --update: computes and validates the entry,
    /// prints it to stdout, but skips the atomic write/replace.
    #[arg(long)]
    pub(super) dry_run: bool,
    /// Apply a versioned add-finding plan produced by `why --plan`. Re-scans the
    /// live source tree, recomputes and verifies every plan binding, requires the
    /// exact finding to remain uniquely `New`, and atomically replaces the live
    /// ledger. Consumes only operator judgment fields (owner/reason/etc.);
    /// target selectors come from the plan. Requires `--update`; conflicts with
    /// manual target selectors, `--write`, and `--force`.
    #[arg(
        long = "from-plan",
        requires = "update",
        conflicts_with_all = ["write", "force", "path", "line", "glob", "family", "callee"]
    )]
    pub(super) from_plan: Option<PathBuf>,
    /// Summary output format. JSON requires --summary-output so it cannot be
    /// mixed with policy or warning text on stderr. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) summary_format: HumanJsonFormat,
    /// Write add summary to a file. Required with --summary-format json.
    #[arg(long)]
    pub(super) summary_output: Option<PathBuf>,
}
