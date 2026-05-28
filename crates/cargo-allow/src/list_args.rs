use allow_core::CargoAllowResult;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, parse_kind_filter};

use super::list_types::ListFilters;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Filter allow entries by kind.
    #[arg(long)]
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
    /// Filter allow entries by current match status.
    #[arg(
        long,
        value_parser = [
            "matched",
            "new",
            "stale",
            "expired",
            "review_due",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt"
        ]
    )]
    pub(super) status: Option<String>,
    /// Include only expired allow entries.
    #[arg(long)]
    pub(super) expired: bool,
    /// Include only review-due allow entries.
    #[arg(long)]
    pub(super) review_due: bool,
    /// Include only stale allow entries.
    #[arg(long)]
    pub(super) stale: bool,
    /// Include only generated baseline debt entries.
    #[arg(long)]
    pub(super) baseline_debt: bool,
    /// Include only entries with wildcard source-tree scopes.
    #[arg(long)]
    pub(super) broad_scope: bool,
    /// Include only entries with no evidence references.
    #[arg(long)]
    pub(super) missing_evidence: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ListFormat::Human)]
    pub(super) format: ListFormat,
    /// Write list output to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Include untracked files when determining current match status.
    #[arg(long)]
    pub(super) include_untracked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum ListFormat {
    Human,
    Json,
}

pub(super) fn list_filters(args: &ListArgs) -> CargoAllowResult<ListFilters<'_>> {
    Ok(ListFilters {
        kind: args.kind.as_deref().map(parse_kind_filter).transpose()?,
        family: args.family.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        status: args.status.as_deref(),
        expired: args.expired,
        review_due: args.review_due,
        stale: args.stale,
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        missing_evidence: args.missing_evidence,
    })
}
