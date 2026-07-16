use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, parse_kind_filter, parse_kind_filter_arg, parse_match_status_arg};

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
    /// Mutually exclusive with `--expired`, `--review-due`, and `--stale`
    /// (pick one status selector). `--baseline-debt` is a classification
    /// filter and may still be combined with `--status`.
    #[arg(long, value_parser = parse_match_status_arg)]
    pub(super) status: Option<String>,
    /// Include only expired allow entries.
    ///
    /// Mutually exclusive with `--status`, `--review-due`, and `--stale`.
    #[arg(long, conflicts_with = "status")]
    pub(super) expired: bool,
    /// Include only review-due allow entries.
    ///
    /// Mutually exclusive with `--status`, `--expired`, and `--stale`.
    #[arg(long, conflicts_with = "status")]
    pub(super) review_due: bool,
    /// Include only stale allow entries.
    ///
    /// Mutually exclusive with `--status`, `--expired`, and `--review-due`.
    #[arg(long, conflicts_with = "status")]
    pub(super) stale: bool,
    /// Include only entries with classification `baseline_debt`.
    ///
    /// Classification filter; may be combined with a single status selector.
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
    validate_status_selectors(args)?;
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
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        missing_evidence: args.missing_evidence,
        broken_evidence: args.broken_evidence,
        weak_evidence: args.weak_evidence,
    })
}

/// Reject conflicting status selectors so list never silently ANDs them to empty.
///
/// Status selectors are `--status`, `--expired`, `--review-due`, and `--stale`.
/// `--baseline-debt` is a classification filter and may combine with one status
/// selector. Clap also rejects `--status` combined with a shortcut at parse
/// time; this check covers conflicting shortcuts and programmatic callers.
fn validate_status_selectors(args: &ListArgs) -> CargoAllowResult<()> {
    let mut selected = Vec::new();
    if args.status.is_some() {
        selected.push("--status");
    }
    if args.expired {
        selected.push("--expired");
    }
    if args.review_due {
        selected.push("--review-due");
    }
    if args.stale {
        selected.push("--stale");
    }
    if selected.len() <= 1 {
        return Ok(());
    }
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!(
            "status filters are mutually exclusive; got {} (choose one of --status, --expired, --review-due, --stale)",
            selected.join(", ")
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RootArgs;

    fn list_args(
        status: Option<&str>,
        expired: bool,
        review_due: bool,
        stale: bool,
        baseline_debt: bool,
    ) -> ListArgs {
        ListArgs {
            root: RootArgs { root: None },
            config: None,
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            allow_id: None,
            status: status.map(str::to_owned),
            expired,
            review_due,
            stale,
            baseline_debt,
            broad_scope: false,
            missing_evidence: false,
            broken_evidence: false,
            weak_evidence: false,
            format: ListFormat::Human,
            output: None,
            include_untracked: false,
        }
    }

    #[test]
    fn list_filters_accepts_single_status_selector() {
        let args = list_args(Some("expired"), false, false, false, false);
        let filters = list_filters(&args).unwrap_or_else(|err| {
            std::panic::panic_any(format!("single --status should be accepted: {err}"))
        });
        assert_eq!(filters.status, Some("expired"));
        assert!(!filters.expired);

        let args = list_args(None, true, false, false, false);
        let filters = list_filters(&args).unwrap_or_else(|err| {
            std::panic::panic_any(format!("single --expired should be accepted: {err}"))
        });
        assert!(filters.expired);
        assert!(filters.status.is_none());
    }

    #[test]
    fn list_filters_allows_status_with_baseline_debt_classification() {
        let args = list_args(Some("expired"), false, false, false, true);
        let filters = list_filters(&args).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "--status with --baseline-debt classification filter should be accepted: {err}"
            ))
        });
        assert_eq!(filters.status, Some("expired"));
        assert!(filters.baseline_debt);
    }

    #[test]
    fn list_filters_rejects_status_with_status_bool() {
        let err = list_filters(&list_args(Some("expired"), false, true, false, false))
            .err()
            .unwrap_or_else(|| {
                std::panic::panic_any("--status with --review-due should fail closed")
            });
        assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
        assert!(err.message().contains("mutually exclusive"));
        assert!(err.message().contains("--status"));
        assert!(err.message().contains("--review-due"));
    }

    #[test]
    fn list_filters_rejects_conflicting_status_bools() {
        let err = list_filters(&list_args(None, true, false, true, false))
            .err()
            .unwrap_or_else(|| std::panic::panic_any("--expired with --stale should fail closed"));
        assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
        assert!(err.message().contains("--expired"));
        assert!(err.message().contains("--stale"));
    }
}
