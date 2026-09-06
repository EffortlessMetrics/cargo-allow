//! Read-only review-readiness check projection (#3844).
//!
//! Projects a retained `ReviewDispositionV1` (or its explicit absence)
//! plus a live source snapshot and the PR Draft/Ready posture onto the
//! stable `review-readiness` check context. The command never contacts
//! GitHub and never mutates PR state; the source-controlled workflow
//! owns event delivery, and #2284 owns live required-control posture.

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    ReviewReadinessConclusionV1, ReviewReadinessDispositionInputV1, ReviewReadinessDraftStateV1,
    ReviewReadinessEventV1, ReviewReadinessProjectionInputV1, evaluate_review_readiness_projection,
    parse_review_disposition_bytes, parse_review_readiness_live_bytes,
    render_review_readiness_human, render_review_readiness_json,
};
use clap::{Parser, Subcommand};

/// Read-only review-readiness projection (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReviewReadinessArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewReadinessSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewReadinessSubcommand {
    /// Project one disposition (or its absence) onto the check context.
    #[command(hide = true)]
    Project(ReviewReadinessProjectArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReviewReadinessDraftStateArg {
    Draft,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReviewReadinessEventArg {
    Opened,
    Reopened,
    Synchronize,
    ForcePush,
    ReadyForReview,
    ConvertedToDraft,
    BaseMoved,
    MergeBaseMoved,
    DispositionUpdated,
    WorkflowConfigMoved,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReviewReadinessProjectArgs {
    /// Retained review disposition JSON path. Omit for the explicit
    /// missing-disposition case.
    #[arg(long)]
    pub(crate) disposition: Option<PathBuf>,
    /// Live source snapshot JSON path.
    #[arg(long)]
    pub(crate) live: PathBuf,
    /// PR Draft/Ready posture.
    #[arg(long)]
    pub(crate) draft_state: ReviewReadinessDraftStateArg,
    /// The triggering readiness-relevant event.
    #[arg(long)]
    pub(crate) event: ReviewReadinessEventArg,
    /// Prior retained check observation JSON path, when one exists.
    #[arg(long)]
    pub(crate) prior: Option<PathBuf>,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: ReviewReadinessOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReviewReadinessOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_review_readiness(args: &ReviewReadinessArgs) -> CargoAllowResult<()> {
    let ReviewReadinessSubcommand::Project(project) = &args.command;

    let live_bytes = std::fs::read(&project.live).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "live source snapshot read {}: {error}",
                project.live.display()
            ),
        )
    })?;
    let live = parse_review_readiness_live_bytes(&live_bytes).map_err(|failure| {
        CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, failure.reason)
    })?;

    let disposition = match &project.disposition {
        None => ReviewReadinessDispositionInputV1::Missing,
        Some(path) => match std::fs::read(path) {
            Err(error) => ReviewReadinessDispositionInputV1::Malformed {
                reason: format!("disposition read {}: {error}", path.display()),
            },
            Ok(bytes) => match parse_review_disposition_bytes(&bytes) {
                Ok(disposition) => {
                    ReviewReadinessDispositionInputV1::Present(Box::new(disposition))
                }
                Err(failure) => ReviewReadinessDispositionInputV1::Malformed {
                    reason: failure.reason,
                },
            },
        },
    };

    let prior_observation = match &project.prior {
        None => None,
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!("prior observation read {}: {error}", path.display()),
                )
            })?;
            Some(serde_json::from_slice(&bytes).map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!("prior observation parse: {error}"),
                )
            })?)
        }
    };

    let event = match project.event {
        ReviewReadinessEventArg::Opened => ReviewReadinessEventV1::Opened,
        ReviewReadinessEventArg::Reopened => ReviewReadinessEventV1::Reopened,
        ReviewReadinessEventArg::Synchronize => ReviewReadinessEventV1::Synchronize,
        ReviewReadinessEventArg::ForcePush => ReviewReadinessEventV1::ForcePush,
        ReviewReadinessEventArg::ReadyForReview => ReviewReadinessEventV1::ReadyForReview,
        ReviewReadinessEventArg::ConvertedToDraft => ReviewReadinessEventV1::ConvertedToDraft,
        ReviewReadinessEventArg::BaseMoved => ReviewReadinessEventV1::BaseMoved,
        ReviewReadinessEventArg::MergeBaseMoved => ReviewReadinessEventV1::MergeBaseMoved,
        ReviewReadinessEventArg::DispositionUpdated => ReviewReadinessEventV1::DispositionUpdated,
        ReviewReadinessEventArg::WorkflowConfigMoved => ReviewReadinessEventV1::WorkflowConfigMoved,
    };
    let draft_state = match project.draft_state {
        ReviewReadinessDraftStateArg::Draft => ReviewReadinessDraftStateV1::Draft,
        ReviewReadinessDraftStateArg::Ready => ReviewReadinessDraftStateV1::Ready,
    };

    let input = ReviewReadinessProjectionInputV1 {
        disposition,
        live,
        draft_state,
        event,
        prior_observation,
    };
    let projection = evaluate_review_readiness_projection(&input);
    match project.format {
        ReviewReadinessOutputFormat::Json => {
            println!(
                "{}",
                render_review_readiness_json(&projection).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("projection serialization: {error}"),
                    )
                })?
            );
        }
        ReviewReadinessOutputFormat::Human => {
            println!("{}", render_review_readiness_human(&projection));
        }
    }

    // Failure is the only job-failing conclusion: a blocked, stale, or
    // malformed review state must be machine-visible. Neutral (missing
    // disposition) stays advisory until #2284 selects the live
    // required-control posture.
    if projection.conclusion == ReviewReadinessConclusionV1::Failure {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "review-readiness conclusion is failure for {}#{}; see printed projection",
                projection.repository, projection.pr_number
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use allow_report::{ReviewTransitionRequestV1, parse_review_transition_request_bytes};

    #[test]
    fn parse_review_transition_request_bytes_is_available_for_adapters() {
        // Adapter-surface check: the readiness question can be built
        // and parsed from bytes by exact adapters.
        let request = ReviewTransitionRequestV1 {
            current_state: allow_report::ReviewReadinessStateV1::Draft,
            target_state: allow_report::ReviewReadinessStateV1::Ready,
            required_checks: Vec::new(),
        };
        let bytes =
            serde_json::to_vec(&request).expect("fixture serialization of a known-good request");
        let parsed = parse_review_transition_request_bytes(&bytes);
        assert!(parsed.is_ok());
    }
}
