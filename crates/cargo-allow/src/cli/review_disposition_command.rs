//! Read-only review-disposition verification (#3843).
//!
//! Evaluates a retained `ReviewDispositionV1` against a live source
//! snapshot and a declared readiness transition request, emitting the
//! bounded `ReviewDispositionOutcomeV1`. All three inputs are JSON
//! files prepared by the caller. The command never contacts GitHub,
//! never mutates PR Draft/Ready state, and never publishes a check;
//! #3844 owns check/control projection and #2284 owns live readback.
//!
//! Exit contract: instrument failures (malformed disposition, snapshot,
//! or request) fail closed, and a requested Draft -> Ready transition
//! that the typed law does not permit fails, so the command is directly
//! usable as a gate. Informative outcomes exit zero.

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    ReviewCurrentnessV1, evaluate_review_disposition, parse_review_disposition_bytes,
    parse_review_live_source_bytes, parse_review_transition_request_bytes,
    render_review_disposition_human, render_review_disposition_json,
};
use clap::{Parser, Subcommand};

/// Read-only review-disposition verification (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReviewDispositionArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewDispositionSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewDispositionSubcommand {
    /// Check one disposition against a live source snapshot and a
    /// readiness transition request.
    #[command(hide = true)]
    Check(ReviewDispositionCheckArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReviewDispositionCheckArgs {
    /// Retained review disposition JSON path.
    #[arg(long)]
    pub(crate) disposition: PathBuf,
    /// Live source snapshot JSON path.
    #[arg(long)]
    pub(crate) live: PathBuf,
    /// Readiness transition request JSON path.
    #[arg(long)]
    pub(crate) request: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: ReviewDispositionOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReviewDispositionOutputFormat {
    Json,
    Human,
}

fn invalid_config(reason: String) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, reason)
}

pub(super) fn cmd_review_disposition(args: &ReviewDispositionArgs) -> CargoAllowResult<()> {
    let ReviewDispositionSubcommand::Check(check) = &args.command;
    let disposition_bytes = std::fs::read(&check.disposition).map_err(|error| {
        invalid_config(format!(
            "review disposition read {}: {error}",
            check.disposition.display()
        ))
    })?;
    let disposition = parse_review_disposition_bytes(&disposition_bytes)
        .map_err(|failure| invalid_config(failure.reason))?;

    let live_bytes = std::fs::read(&check.live).map_err(|error| {
        invalid_config(format!(
            "live source snapshot read {}: {error}",
            check.live.display()
        ))
    })?;
    let live = parse_review_live_source_bytes(&live_bytes)
        .map_err(|failure| invalid_config(failure.reason))?;

    let request_bytes = std::fs::read(&check.request).map_err(|error| {
        invalid_config(format!(
            "transition request read {}: {error}",
            check.request.display()
        ))
    })?;
    let request = parse_review_transition_request_bytes(&request_bytes)
        .map_err(|failure| invalid_config(failure.reason))?;

    let outcome = evaluate_review_disposition(&disposition, &live, &request);
    match check.format {
        ReviewDispositionOutputFormat::Json => {
            println!(
                "{}",
                render_review_disposition_json(&outcome).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("result serialization: {error}"),
                    )
                })?
            );
        }
        ReviewDispositionOutputFormat::Human => {
            println!("{}", render_review_disposition_human(&outcome));
        }
    }

    // Instrument failures fail closed; a readiness move the typed law
    // does not permit fails so the command is usable as a gate by the
    // check/control projection (#3844).
    if outcome.currentness == ReviewCurrentnessV1::InstrumentFailure {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            format!(
                "review disposition for {}#{} is an instrument failure; see printed outcome",
                outcome.repository, outcome.pr_number
            ),
        ));
    }
    if !outcome.transition.permitted {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "transition {} -> {} is not permitted for {}#{}; see printed outcome",
                outcome.transition.from_state.label(),
                outcome.transition.to_state.label(),
                outcome.repository,
                outcome.pr_number
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use allow_report::{
        CampaignCheckOutcomeV1, CampaignEvidenceClassV1, IndependentReviewPostureV1,
        ReviewActorClassV1, ReviewCheckObservationV1, ReviewFindingSeverityV1, ReviewFindingV1,
        ReviewReadinessStateV1, ReviewRequiredCiV1, ReviewTransitionRequestV1,
    };
    use allow_report::{
        ReviewCurrentnessV1, ReviewDispositionV1, ReviewLiveSourceV1, evaluate_review_disposition,
    };

    use super::*;

    fn disposition() -> ReviewDispositionV1 {
        ReviewDispositionV1 {
            schema_id: "cargo-allow.review-disposition.v1".to_string(),
            schema_version: 1,
            repository: "owner/repo".to_string(),
            pr_number: 4143,
            base_ref: "main".to_string(),
            base_sha: "aaaa".to_string(),
            head_ref: "test/review-disposition-schema".to_string(),
            head_sha: "bbbb".to_string(),
            merge_base: "aaaa".to_string(),
            reviewed_diff_digest: "sha256:v1:digest".to_string(),
            review_protocol: "review-current-head-gen1".to_string(),
            actor_class: ReviewActorClassV1::SameMaintainer,
            reviewer_identity: "solo-maintainer".to_string(),
            independent_review: IndependentReviewPostureV1::NotRetained,
            claimed_verdict: ReviewCurrentnessV1::ReviewClean,
            findings: vec![ReviewFindingV1 {
                id: "ADV-001".to_string(),
                severity: ReviewFindingSeverityV1::Advisory,
                owned_seam: "docs".to_string(),
                source_path: "docs/x.md".to_string(),
                source_line: Some(3),
                repair_route: "doc follow-up".to_string(),
                claim_boundary: "advisory only".to_string(),
            }],
            threads_inspected: vec!["thread-1".to_string()],
            required_ci: ReviewRequiredCiV1 {
                owner: "check-projection-3844".to_string(),
                observation_ref: String::new(),
            },
            evidence_class: CampaignEvidenceClassV1::CurrentObservation,
            scope_claim_boundary: "issue:3843".to_string(),
            reviewed_at_utc: "2026-09-05T00:00:00Z".to_string(),
        }
    }

    fn live() -> ReviewLiveSourceV1 {
        ReviewLiveSourceV1 {
            repository: "owner/repo".to_string(),
            pr_number: 4143,
            base_ref: "main".to_string(),
            base_sha: "aaaa".to_string(),
            head_ref: "test/review-disposition-schema".to_string(),
            head_sha: "bbbb".to_string(),
            merge_base: "aaaa".to_string(),
            diff_digest: "sha256:v1:digest".to_string(),
            review_protocol: "review-current-head-gen1".to_string(),
            scope_claim_boundary: "issue:3843".to_string(),
        }
    }

    fn request() -> ReviewTransitionRequestV1 {
        ReviewTransitionRequestV1 {
            current_state: ReviewReadinessStateV1::Draft,
            target_state: ReviewReadinessStateV1::Ready,
            required_checks: vec![ReviewCheckObservationV1 {
                name: "ci".to_string(),
                outcome: CampaignCheckOutcomeV1::Passed,
            }],
        }
    }

    fn write_json(name: &str, value: &serde_json::Value) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "review-disposition-cmd-{name}-{}.json",
            std::process::id()
        ));
        let bytes = serde_json::to_vec_pretty(value).expect("serialize fixture");
        std::fs::write(&path, bytes).expect("write fixture");
        path
    }

    fn cleanup(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn check_permits_a_current_clean_transition() {
        let disposition_path = write_json(
            "ok-disposition",
            &serde_json::to_value(disposition()).expect("value"),
        );
        let live_path = write_json("ok-live", &serde_json::to_value(live()).expect("value"));
        let request_path = write_json(
            "ok-request",
            &serde_json::to_value(request()).expect("value"),
        );
        let args = ReviewDispositionArgs {
            command: ReviewDispositionSubcommand::Check(ReviewDispositionCheckArgs {
                disposition: disposition_path.clone(),
                live: live_path.clone(),
                request: request_path.clone(),
                format: ReviewDispositionOutputFormat::Human,
            }),
        };
        let outcome = cmd_review_disposition(&args);
        cleanup(&disposition_path);
        cleanup(&live_path);
        cleanup(&request_path);
        assert!(outcome.is_ok());
    }

    #[test]
    fn check_fails_when_the_transition_is_not_permitted() {
        let mut moved = live();
        moved.head_sha = "ffff".to_string();
        let disposition_path = write_json(
            "blocked-disposition",
            &serde_json::to_value(disposition()).expect("value"),
        );
        let live_path = write_json("blocked-live", &serde_json::to_value(moved).expect("value"));
        let request_path = write_json(
            "blocked-request",
            &serde_json::to_value(request()).expect("value"),
        );
        let args = ReviewDispositionArgs {
            command: ReviewDispositionSubcommand::Check(ReviewDispositionCheckArgs {
                disposition: disposition_path.clone(),
                live: live_path.clone(),
                request: request_path.clone(),
                format: ReviewDispositionOutputFormat::Json,
            }),
        };
        let outcome = cmd_review_disposition(&args);
        cleanup(&disposition_path);
        cleanup(&live_path);
        cleanup(&request_path);
        assert!(outcome.is_err());
    }

    #[test]
    fn check_fails_closed_on_a_malformed_disposition() {
        let mut malformed = disposition();
        malformed.schema_id = "not-the-schema".to_string();
        let disposition_path = write_json(
            "bad-disposition",
            &serde_json::to_value(malformed).expect("value"),
        );
        let live_path = write_json("bad-live", &serde_json::to_value(live()).expect("value"));
        let request_path = write_json(
            "bad-request",
            &serde_json::to_value(request()).expect("value"),
        );
        let args = ReviewDispositionArgs {
            command: ReviewDispositionSubcommand::Check(ReviewDispositionCheckArgs {
                disposition: disposition_path.clone(),
                live: live_path.clone(),
                request: request_path.clone(),
                format: ReviewDispositionOutputFormat::Json,
            }),
        };
        let outcome = cmd_review_disposition(&args);
        cleanup(&disposition_path);
        cleanup(&live_path);
        cleanup(&request_path);
        assert!(outcome.is_err());
    }

    #[test]
    fn evaluate_is_pure_across_repeated_calls() {
        let outcome_one = evaluate_review_disposition(&disposition(), &live(), &request());
        let outcome_two = evaluate_review_disposition(&disposition(), &live(), &request());
        assert_eq!(outcome_one, outcome_two);
    }
}
