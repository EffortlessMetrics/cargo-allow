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
    use allow_report::{
        CampaignEvidenceClassV1, IndependentReviewPostureV1, ReviewActorClassV1,
        ReviewCurrentnessV1, ReviewDispositionV1, ReviewTransitionRequestV1,
        parse_review_transition_request_bytes,
    };

    use super::*;

    fn disposition() -> ReviewDispositionV1 {
        ReviewDispositionV1 {
            schema_id: "cargo-allow.review-disposition.v1".to_string(),
            schema_version: 1,
            repository: "owner/repo".to_string(),
            pr_number: 4146,
            base_ref: "main".to_string(),
            base_sha: "aaaa".to_string(),
            head_ref: "test/review-readiness-check".to_string(),
            head_sha: "bbbb".to_string(),
            merge_base: "aaaa".to_string(),
            reviewed_diff_digest: "sha256:v1:digest".to_string(),
            review_protocol: "review-current-head-gen1".to_string(),
            actor_class: ReviewActorClassV1::SameMaintainer,
            reviewer_identity: "solo-maintainer".to_string(),
            independent_review: IndependentReviewPostureV1::NotRetained,
            claimed_verdict: ReviewCurrentnessV1::ReviewClean,
            findings: Vec::new(),
            threads_inspected: Vec::new(),
            required_ci: allow_report::ReviewRequiredCiV1 {
                owner: "check-projection-3844".to_string(),
                observation_ref: String::new(),
            },
            evidence_class: CampaignEvidenceClassV1::CurrentObservation,
            scope_claim_boundary: "issue:3844".to_string(),
            reviewed_at_utc: "2026-09-06T00:00:00Z".to_string(),
        }
    }

    fn live() -> allow_report::ReviewLiveSourceV1 {
        allow_report::ReviewLiveSourceV1 {
            repository: "owner/repo".to_string(),
            pr_number: 4146,
            base_ref: "main".to_string(),
            base_sha: "aaaa".to_string(),
            head_ref: "test/review-readiness-check".to_string(),
            head_sha: "bbbb".to_string(),
            merge_base: "aaaa".to_string(),
            diff_digest: "sha256:v1:digest".to_string(),
            review_protocol: "review-current-head-gen1".to_string(),
            scope_claim_boundary: "issue:3844".to_string(),
        }
    }

    fn write_json(tag: &str, value: &serde_json::Value) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "review-readiness-cmd-{tag}-{}.json",
            std::process::id()
        ));
        let bytes = serde_json::to_vec_pretty(value).expect("fixture serialization succeeds");
        std::fs::write(&path, bytes).expect("fixture write succeeds");
        path
    }

    fn cleanup(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
    }

    fn args(
        disposition: Option<PathBuf>,
        live_path: PathBuf,
        draft_state: ReviewReadinessDraftStateArg,
        event: ReviewReadinessEventArg,
        prior: Option<PathBuf>,
        format: ReviewReadinessOutputFormat,
    ) -> ReviewReadinessArgs {
        ReviewReadinessArgs {
            command: ReviewReadinessSubcommand::Project(ReviewReadinessProjectArgs {
                disposition,
                live: live_path,
                draft_state,
                event,
                prior,
                format,
            }),
        }
    }

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

    #[test]
    fn project_succeeds_for_a_current_clean_disposition_in_both_formats() {
        let disposition_path = write_json(
            "clean",
            &serde_json::to_value(disposition()).expect("value"),
        );
        let live_path = write_json("clean-live", &serde_json::to_value(live()).expect("value"));
        for format in [
            ReviewReadinessOutputFormat::Json,
            ReviewReadinessOutputFormat::Human,
        ] {
            let outcome = cmd_review_readiness(&args(
                Some(disposition_path.clone()),
                live_path.clone(),
                ReviewReadinessDraftStateArg::Ready,
                ReviewReadinessEventArg::ReadyForReview,
                None,
                format,
            ));
            assert!(outcome.is_ok());
        }
        cleanup(&disposition_path);
        cleanup(&live_path);
    }

    #[test]
    fn project_fails_for_a_blocked_disposition() {
        let mut blocked = disposition();
        blocked.claimed_verdict = ReviewCurrentnessV1::ReviewBlocked;
        blocked.findings = vec![allow_report::ReviewFindingV1 {
            id: "BLK-001".to_string(),
            severity: allow_report::ReviewFindingSeverityV1::Blocking,
            owned_seam: "seam".to_string(),
            source_path: "path.rs".to_string(),
            source_line: Some(1),
            repair_route: "repair".to_string(),
            claim_boundary: "blocking".to_string(),
        }];
        let disposition_path =
            write_json("blocked", &serde_json::to_value(blocked).expect("value"));
        let live_path = write_json(
            "blocked-live",
            &serde_json::to_value(live()).expect("value"),
        );
        let outcome = cmd_review_readiness(&args(
            Some(disposition_path.clone()),
            live_path.clone(),
            ReviewReadinessDraftStateArg::Ready,
            ReviewReadinessEventArg::Synchronize,
            None,
            ReviewReadinessOutputFormat::Human,
        ));
        cleanup(&disposition_path);
        cleanup(&live_path);
        assert!(outcome.is_err());
    }

    #[test]
    fn project_stays_neutral_without_a_disposition() {
        let live_path = write_json(
            "missing-live",
            &serde_json::to_value(live()).expect("value"),
        );
        let outcome = cmd_review_readiness(&args(
            None,
            live_path.clone(),
            ReviewReadinessDraftStateArg::Ready,
            ReviewReadinessEventArg::ConvertedToDraft,
            None,
            ReviewReadinessOutputFormat::Json,
        ));
        cleanup(&live_path);
        assert!(outcome.is_ok());
    }

    #[test]
    fn project_fails_closed_on_a_malformed_disposition_file() {
        let malformed_path = std::env::temp_dir().join(format!(
            "review-readiness-cmd-malformed-{}.json",
            std::process::id()
        ));
        std::fs::write(&malformed_path, br#"{"comment": "LGTM clean"}"#)
            .expect("fixture write succeeds");
        let live_path = write_json(
            "malformed-live",
            &serde_json::to_value(live()).expect("value"),
        );
        let outcome = cmd_review_readiness(&args(
            Some(malformed_path.clone()),
            live_path.clone(),
            ReviewReadinessDraftStateArg::Draft,
            ReviewReadinessEventArg::Opened,
            None,
            ReviewReadinessOutputFormat::Json,
        ));
        cleanup(&malformed_path);
        cleanup(&live_path);
        assert!(outcome.is_err());
    }

    #[test]
    fn project_maps_a_missing_live_file_to_a_config_error() {
        let absent = std::env::temp_dir().join(format!(
            "review-readiness-cmd-absent-live-{}.json",
            std::process::id()
        ));
        let outcome = cmd_review_readiness(&args(
            None,
            absent,
            ReviewReadinessDraftStateArg::Draft,
            ReviewReadinessEventArg::Opened,
            None,
            ReviewReadinessOutputFormat::Json,
        ));
        assert!(outcome.is_err());
    }

    #[test]
    fn project_consumes_a_prior_observation() {
        let prior = serde_json::json!({
            "conclusion": "success",
            "binding": {
                "repository": "owner/repo",
                "pr_number": 4146,
                "base_ref": "main",
                "base_sha": "aaaa",
                "head_ref": "test/review-readiness-check",
                "head_sha": "bbbb",
                "merge_base": "aaaa",
                "diff_digest": "sha256:v1:digest",
                "disposition_identity": "fnv1a64:previous"
            }
        });
        let prior_path = write_json("prior", &prior);
        let disposition_path = write_json(
            "prior-disposition",
            &serde_json::to_value(disposition()).expect("value"),
        );
        let live_path = write_json("prior-live", &serde_json::to_value(live()).expect("value"));
        let outcome = cmd_review_readiness(&args(
            Some(disposition_path.clone()),
            live_path.clone(),
            ReviewReadinessDraftStateArg::Ready,
            ReviewReadinessEventArg::Reopened,
            Some(prior_path.clone()),
            ReviewReadinessOutputFormat::Human,
        ));
        cleanup(&prior_path);
        cleanup(&disposition_path);
        cleanup(&live_path);
        assert!(outcome.is_ok());
    }

    #[test]
    fn project_fails_closed_on_a_malformed_prior_observation() {
        let prior_path = std::env::temp_dir().join(format!(
            "review-readiness-cmd-bad-prior-{}.json",
            std::process::id()
        ));
        std::fs::write(&prior_path, br#"{"conclusion": "nonsense"}"#)
            .expect("fixture write succeeds");
        let live_path = write_json(
            "bad-prior-live",
            &serde_json::to_value(live()).expect("value"),
        );
        let outcome = cmd_review_readiness(&args(
            None,
            live_path.clone(),
            ReviewReadinessDraftStateArg::Draft,
            ReviewReadinessEventArg::Opened,
            Some(prior_path.clone()),
            ReviewReadinessOutputFormat::Json,
        ));
        cleanup(&prior_path);
        cleanup(&live_path);
        assert!(outcome.is_err());
    }

    #[test]
    fn project_fails_closed_on_an_unreadable_disposition_path() {
        let absent = std::env::temp_dir().join(format!(
            "review-readiness-cmd-absent-disposition-{}.json",
            std::process::id()
        ));
        let live_path = write_json(
            "absent-disposition-live",
            &serde_json::to_value(live()).expect("value"),
        );
        let outcome = cmd_review_readiness(&args(
            Some(absent),
            live_path.clone(),
            ReviewReadinessDraftStateArg::Draft,
            ReviewReadinessEventArg::Opened,
            None,
            ReviewReadinessOutputFormat::Json,
        ));
        cleanup(&live_path);
        assert!(outcome.is_err());
    }
}
