//! Event and freshness-law tests for the #3844 review-readiness
//! check: every readiness-relevant event recomputes, and a retained
//! green bound to a moved pair is a stale green.

use allow_report::{
    CampaignEvidenceClassV1, IndependentReviewPostureV1, ReviewActorClassV1, ReviewCurrentnessV1,
    ReviewDispositionV1, ReviewLiveSourceV1, ReviewReadinessBindingV1, ReviewReadinessConclusionV1,
    ReviewReadinessDispositionInputV1, ReviewReadinessDraftStateV1, ReviewReadinessEventV1,
    ReviewReadinessObservationV1, ReviewReadinessProjectionInputV1, ReviewReadinessStateV1,
    evaluate_review_readiness_projection,
};

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

fn live() -> ReviewLiveSourceV1 {
    ReviewLiveSourceV1 {
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

fn green_binding() -> ReviewReadinessBindingV1 {
    ReviewReadinessBindingV1 {
        repository: "owner/repo".to_string(),
        pr_number: 4146,
        base_ref: "main".to_string(),
        base_sha: "aaaa".to_string(),
        head_ref: "test/review-readiness-check".to_string(),
        head_sha: "bbbb".to_string(),
        merge_base: "aaaa".to_string(),
        diff_digest: "sha256:v1:digest".to_string(),
        disposition_identity: "fnv1a64:previous".to_string(),
    }
}

fn input(
    disposition_input: ReviewReadinessDispositionInputV1,
    event: ReviewReadinessEventV1,
    live_source: ReviewLiveSourceV1,
    prior: Option<ReviewReadinessObservationV1>,
) -> ReviewReadinessProjectionInputV1 {
    ReviewReadinessProjectionInputV1 {
        disposition: disposition_input,
        live: live_source,
        draft_state: ReviewReadinessDraftStateV1::Ready,
        event,
        prior_observation: prior,
    }
}

#[test]
fn review_readiness_events_recompute_on_every_readiness_relevant_event() {
    for event in [
        ReviewReadinessEventV1::Opened,
        ReviewReadinessEventV1::Reopened,
        ReviewReadinessEventV1::Synchronize,
        ReviewReadinessEventV1::ForcePush,
        ReviewReadinessEventV1::ReadyForReview,
        ReviewReadinessEventV1::ConvertedToDraft,
        ReviewReadinessEventV1::BaseMoved,
        ReviewReadinessEventV1::MergeBaseMoved,
        ReviewReadinessEventV1::DispositionUpdated,
        ReviewReadinessEventV1::WorkflowConfigMoved,
    ] {
        let projection = evaluate_review_readiness_projection(&input(
            ReviewReadinessDispositionInputV1::Present(Box::new(disposition())),
            event,
            live(),
            None,
        ));
        assert_eq!(
            projection.conclusion,
            ReviewReadinessConclusionV1::Success,
            "event {} must recompute from structured state",
            event.label()
        );
        assert_eq!(projection.event, event);
    }
}

#[test]
fn review_readiness_events_invalidate_a_green_bound_to_a_moved_pair() {
    for event in [
        ReviewReadinessEventV1::Synchronize,
        ReviewReadinessEventV1::ForcePush,
        ReviewReadinessEventV1::BaseMoved,
        ReviewReadinessEventV1::MergeBaseMoved,
        ReviewReadinessEventV1::DispositionUpdated,
        ReviewReadinessEventV1::WorkflowConfigMoved,
    ] {
        let mut moved = live();
        moved.head_sha = "cccc".to_string();
        let prior = ReviewReadinessObservationV1 {
            conclusion: ReviewReadinessConclusionV1::Success,
            binding: green_binding(),
        };
        let projection = evaluate_review_readiness_projection(&input(
            ReviewReadinessDispositionInputV1::Missing,
            event,
            moved,
            Some(prior),
        ));
        assert!(
            projection.stale_green_invalidated,
            "event {} must invalidate the old green",
            event.label()
        );
        assert_ne!(
            projection.conclusion,
            ReviewReadinessConclusionV1::Success,
            "an invalidated green must never become the new conclusion on missing state"
        );
    }
}

#[test]
fn review_readiness_events_keep_a_green_current_across_posture_only_events() {
    for event in [
        ReviewReadinessEventV1::ReadyForReview,
        ReviewReadinessEventV1::ConvertedToDraft,
        ReviewReadinessEventV1::Opened,
        ReviewReadinessEventV1::Reopened,
    ] {
        let prior = ReviewReadinessObservationV1 {
            conclusion: ReviewReadinessConclusionV1::Success,
            binding: green_binding(),
        };
        let projection = evaluate_review_readiness_projection(&input(
            ReviewReadinessDispositionInputV1::Present(Box::new(disposition())),
            event,
            live(),
            Some(prior),
        ));
        assert!(!projection.stale_green_invalidated);
        assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Success);
    }
}

#[test]
fn review_readiness_events_fail_a_repaired_head_marked_ready_without_a_fresh_review() {
    // The review covered bbbb; the author force-pushed cccc and marked
    // the PR ready. The old disposition is stale and the check fails.
    let mut repaired = live();
    repaired.head_sha = "cccc".to_string();
    repaired.diff_digest = "sha256:v1:repaired".to_string();
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(disposition())),
        ReviewReadinessEventV1::ForcePush,
        repaired,
        None,
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
    assert_eq!(projection.required_posture, ReviewReadinessStateV1::Draft);
}

#[test]
fn review_readiness_events_reproduce_known_blocked_merge_classes() {
    // #3729/#3743/#3749/#3758 class: a merge attempt while a blocking
    // review stands, with everything else green.
    let mut blocked = disposition();
    blocked.claimed_verdict = ReviewCurrentnessV1::ReviewBlocked;
    blocked.findings = vec![allow_report::ReviewFindingV1 {
        id: "BLK-3729".to_string(),
        severity: allow_report::ReviewFindingSeverityV1::Blocking,
        owned_seam: "crates/allow-match".to_string(),
        source_path: "crates/allow-match/src/lib.rs".to_string(),
        source_line: Some(9),
        repair_route: "repair lane".to_string(),
        claim_boundary: "blocking".to_string(),
    }];
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(blocked)),
        ReviewReadinessEventV1::Synchronize,
        live(),
        None,
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
    assert_eq!(
        projection.required_posture,
        ReviewReadinessStateV1::Draft,
        "a blocked review requires the ready PR to become draft"
    );

    // Stale-green class: a prior green from a pair that has since
    // moved must not survive the next event.
    let mut moved = live();
    moved.head_sha = "cccc".to_string();
    let prior = ReviewReadinessObservationV1 {
        conclusion: ReviewReadinessConclusionV1::Success,
        binding: green_binding(),
    };
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Missing,
        ReviewReadinessEventV1::Synchronize,
        moved,
        Some(prior),
    ));
    assert!(projection.stale_green_invalidated);
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Neutral);

    // Ready-without-review class: marking a PR ready with no retained
    // disposition never reads as clean.
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Missing,
        ReviewReadinessEventV1::ReadyForReview,
        live(),
        None,
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Neutral);
    assert_eq!(projection.required_posture, ReviewReadinessStateV1::Draft);
}
