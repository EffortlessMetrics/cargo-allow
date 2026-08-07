use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, ProfileArg, RootArgs, parse_kind_filter_arg, parse_match_status_arg};

use super::worklist_item_kind::parse_work_item_kind_filter;
use super::worklist_types::WorklistFilters;

#[derive(Debug, Clone, Parser)]
pub(crate) struct WorklistArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Optional governance profile to run instead of the source-exception worklist.
    #[arg(long, value_enum)]
    pub(super) profile: Option<ProfileArg>,
    /// Filter findings by kind.
    #[arg(long, value_parser = parse_worklist_kind_filter)]
    pub(super) kind: Option<String>,
    /// Filter work items by scanner or policy family.
    #[arg(long)]
    pub(super) family: Option<String>,
    /// Filter work items by queue item kind, such as stale_allow or baseline_debt.
    #[arg(long, value_parser = parse_work_item_kind_filter)]
    pub(super) item_kind: Option<String>,
    /// Filter work items by match status.
    ///
    /// Accepts every `MatchStatus` value, including `location_drift`.
    #[arg(long, value_parser = parse_match_status_arg)]
    pub(super) status: Option<String>,
    /// Filter work items by durable allow entry ID.
    #[arg(long)]
    pub(super) allow_id: Option<String>,
    /// Filter work items by source-tree path or path prefix.
    #[arg(long)]
    pub(super) path: Option<String>,
    /// Filter work items by scanner-provided source-tree package context.
    #[arg(long)]
    pub(super) source_package: Option<String>,
    /// Filter work items by policy owner.
    #[arg(long)]
    pub(super) owner: Option<String>,
    /// Filter work items by policy classification.
    #[arg(long)]
    pub(super) classification: Option<String>,
    /// Include only generated baseline debt work items.
    #[arg(long)]
    pub(super) baseline_debt: bool,
    /// Include only broad source-tree scope advisory work items.
    #[arg(long)]
    pub(super) broad_scope: bool,
    /// Filter work items by risk.
    #[arg(long, value_parser = ["low", "medium", "high"])]
    pub(super) risk: Option<String>,
    /// Filter work items by estimated difficulty.
    #[arg(long, value_parser = ["small", "medium"])]
    pub(super) difficulty: Option<String>,
    /// Include only policy-backed work items with no evidence references.
    #[arg(long)]
    pub(super) missing_evidence: bool,
    /// Include only broken local evidence reference work items.
    #[arg(long)]
    pub(super) broken_evidence: bool,
    /// Include only weak evidence reference work items.
    #[arg(long)]
    pub(super) weak_evidence: bool,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) format: HumanJsonFormat,
    /// Write worklist to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
}

fn parse_worklist_kind_filter(value: &str) -> Result<String, String> {
    parse_kind_filter_arg(value)
}

pub(super) fn worklist_filters(args: &WorklistArgs) -> WorklistFilters<'_> {
    WorklistFilters {
        kind: args.kind.as_deref(),
        family: args.family.as_deref(),
        item_kind: args.item_kind.as_deref(),
        status: args.status.as_deref(),
        allow_id: args.allow_id.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        risk: args.risk.as_deref(),
        difficulty: args.difficulty.as_deref(),
        missing_evidence: args.missing_evidence,
        broken_evidence: args.broken_evidence,
        weak_evidence: args.weak_evidence,
    }
}

#[cfg(test)]
#[path = "worklist_args_tests.rs"]
mod tests;
