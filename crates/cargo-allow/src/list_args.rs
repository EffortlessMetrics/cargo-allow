use allow_core::CargoAllowResult;
use clap::Parser;
use std::path::PathBuf;

use crate::{
    HumanJsonFormat, RootArgs, parse_kind_filter, parse_kind_filter_arg, parse_match_status_arg,
};

use super::list_types::ListFilters;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Filter allow entries by kind.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(super) kind: Option<String>,
    /// Filter allow entries by scanner or policy family.
    #[arg(long)]
    pub(super) family: Option<String>,
    /// Filter allow entries by owner.
    #[arg(long)]
    pub(super) owner: Option<String>,
    /// Filter allow entries by classification.
    #[arg(long)]
    pub(super) classification: Option<String>,
    /// Filter allow entries by source-tree path or path prefix.
    #[arg(long)]
    pub(super) path: Option<String>,
    /// Filter allow entries by scanner-provided source-tree package context.
    #[arg(long)]
    pub(super) source_package: Option<String>,
    /// Filter allow entries by durable allow ID.
    #[arg(long)]
    pub(super) allow_id: Option<String>,
    /// Filter allow entries by current match status.
    ///
    /// Accepts every `MatchStatus` value, including `location_drift`.
    /// Mutually exclusive with the status shortcut flags `--expired`,
    /// `--review-due`, `--stale`, and `--location-drift` (pick one status
    /// selector). `--baseline-debt` is a classification filter and may still
    /// be combined with one status selector.
    #[arg(long, group = "status_selector", value_parser = parse_match_status_arg)]
    pub(super) status: Option<String>,
    /// Include only expired allow entries.
    ///
    /// Mutually exclusive with other status selectors.
    #[arg(long, group = "status_selector")]
    pub(super) expired: bool,
    /// Include only review-due allow entries.
    ///
    /// Mutually exclusive with other status selectors.
    #[arg(long, group = "status_selector")]
    pub(super) review_due: bool,
    /// Include only stale allow entries.
    ///
    /// Mutually exclusive with other status selectors.
    #[arg(long, group = "status_selector")]
    pub(super) stale: bool,
    /// Include only location-drift allow entries.
    ///
    /// Mutually exclusive with other status selectors. Same as
    /// `--status location_drift`.
    #[arg(long, group = "status_selector")]
    pub(super) location_drift: bool,
    /// Include only generated baseline debt entries (classification filter).
    ///
    /// May be combined with `--status` (AND). Prefer `--status baseline_debt`
    /// when filtering by match status rather than classification.
    #[arg(long)]
    pub(super) baseline_debt: bool,
    /// Include only entries with wildcard source-tree scopes.
    #[arg(long)]
    pub(super) broad_scope: bool,
    /// Include only entries with no evidence references.
    #[arg(long)]
    pub(super) missing_evidence: bool,
    /// Include only entries with broken local evidence references.
    #[arg(long)]
    pub(super) broken_evidence: bool,
    /// Include only entries with weak evidence references.
    #[arg(long)]
    pub(super) weak_evidence: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) format: HumanJsonFormat,
    /// Select a subset of columns for the human format (comma-separated,
    /// case-insensitive). Example: --columns id,status,reason. Ignored for
    /// --format json. Available columns: id, status, matches, kind, family,
    /// owner, classification, scope, source_package, evidence_count,
    /// broken_evidence_references, weak_evidence_references,
    /// selector_precision, broad_scope, review_after, expires, reason. Use
    /// `--columns all` or `--wide` for the complete human projection.
    #[arg(long, value_name = "COLS", conflicts_with = "wide")]
    pub(super) columns: Option<String>,
    /// Show the complete human projection instead of the concise default.
    /// Equivalent to `--columns all`; JSON is always complete.
    #[arg(long, conflicts_with = "columns")]
    pub(super) wide: bool,
    /// Write list output to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Include untracked files when determining current match status.
    #[arg(long)]
    pub(super) include_untracked: bool,
}

pub(super) fn list_filters(args: &ListArgs) -> CargoAllowResult<ListFilters<'_>> {
    Ok(ListFilters {
        kind: args.kind.as_deref().map(parse_kind_filter).transpose()?,
        family: args.family.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        allow_id: args.allow_id.as_deref(),
        status: args.status.as_deref(),
        expired: args.expired,
        review_due: args.review_due,
        stale: args.stale,
        location_drift: args.location_drift,
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        missing_evidence: args.missing_evidence,
        broken_evidence: args.broken_evidence,
        weak_evidence: args.weak_evidence,
    })
}
