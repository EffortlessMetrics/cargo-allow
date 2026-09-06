//! Readiness-transition-law tests for the #3843 review-disposition
//! model: Ready/Draft transitions consume typed currentness, never
//! prose; CI never overrides a blocked review; stale, partial,
//! unsupported, and instrument results never restore Ready.

use allow_report::{
    CampaignCheckOutcomeV1, CampaignEvidenceClassV1, IndependentReviewPostureV1,
    ReviewActorClassV1, ReviewCheckObservationV1, ReviewCurrentnessV1, ReviewDispositionV1,
    ReviewFindingSeverityV1, ReviewFindingV1, ReviewLiveSourceV1, ReviewReadinessStateV1,
    ReviewRequiredCiV1, ReviewTransitionRequestV1, evaluate_review_disposition,
};

fn disposition_with(claimed: ReviewCurrentnessV1) -> ReviewDispositionV1 {
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
        claimed_verdict: claimed,
        findings: Vec::new(),
        threads_inspected: Vec::new(),
        required_ci: ReviewRequiredCiV1 {
            owner: "check-projection-3844".to_string(),
            observation_ref: String::new(),
        },
        evidence_class: CampaignEvidenceClassV1::CurrentObservation,
        scope_claim_boundary: "issue:3843".to_string(),
        reviewed_at_utc: "2026-09-05T00:00:00Z".to_string(),
    }
}

fn blocking_disposition() -> ReviewDispositionV1 {
    let mut blocked = disposition_with(ReviewCurrentnessV1::ReviewBlocked);
    blocked.findings = vec![ReviewFindingV1 {
        id: "BLK-001".to_string(),
        severity: ReviewFindingSeverityV1::Blocking,
        owned_seam: "crates/allow-match".to_string(),
        source_path: "crates/allow-match/src/lib.rs".to_string(),
        source_line: Some(40),
        repair_route: "repair lane on the same head".to_string(),
        claim_boundary: "blocking".to_string(),
    }];
    blocked
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

fn request(
    from: ReviewReadinessStateV1,
    to: ReviewReadinessStateV1,
    checks: Vec<ReviewCheckObservationV1>,
) -> ReviewTransitionRequestV1 {
    ReviewTransitionRequestV1 {
        current_state: from,
        target_state: to,
        required_checks: checks,
    }
}

fn passed(name: &str) -> ReviewCheckObservationV1 {
    ReviewCheckObservationV1 {
        name: name.to_string(),
        outcome: CampaignCheckOutcomeV1::Passed,
    }
}

#[test]
fn review_readiness_transition_never_lets_ci_green_override_a_blocked_review() {
    let outcome = evaluate_review_disposition(
        &blocking_disposition(),
        &live(),
        &request(
            ReviewReadinessStateV1::Draft,
            ReviewReadinessStateV1::Ready,
            vec![passed("ci"), passed("codecov/patch")],
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::ReviewBlocked);
    assert!(!outcome.transition.permitted);
    assert!(
        outcome
            .transition
            .reasons
            .iter()
            .any(|reason| reason.contains("never overrides a blocked review"))
    );
}

#[test]
fn review_readiness_transition_requires_a_fresh_disposition_after_an_author_repair() {
    let mut repaired = live();
    repaired.head_sha = "cccc".to_string();
    repaired.diff_digest = "sha256:v1:repaired".to_string();
    let outcome = evaluate_review_disposition(
        &disposition_with(ReviewCurrentnessV1::ReviewClean),
        &repaired,
        &request(
            ReviewReadinessStateV1::Draft,
            ReviewReadinessStateV1::Ready,
            vec![passed("ci")],
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        !outcome.transition.permitted,
        "an author 'fixed' statement cannot restore ready; a fresh disposition on the new pair must"
    );
    assert!(
        outcome
            .transition
            .reasons
            .iter()
            .any(|reason| reason.contains("fresh disposition"))
    );
}

#[test]
fn review_readiness_transition_fails_closed_without_a_typed_disposition() {
    let mut unreviewed = disposition_with(ReviewCurrentnessV1::ReviewClean);
    unreviewed.head_sha = String::new();
    let outcome = evaluate_review_disposition(
        &unreviewed,
        &live(),
        &request(
            ReviewReadinessStateV1::Draft,
            ReviewReadinessStateV1::Ready,
            vec![passed("ci")],
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
    assert!(
        !outcome.transition.permitted,
        "the Draft label alone is never substantive review evidence"
    );
}

#[test]
fn review_readiness_transition_never_restores_ready_from_derived_non_clean_states() {
    for (disposition, expected) in [
        (blocking_disposition(), ReviewCurrentnessV1::ReviewBlocked),
        (
            disposition_with(ReviewCurrentnessV1::Partial),
            ReviewCurrentnessV1::Partial,
        ),
        (
            disposition_with(ReviewCurrentnessV1::Unsupported),
            ReviewCurrentnessV1::Unsupported,
        ),
    ] {
        let outcome = evaluate_review_disposition(
            &disposition,
            &live(),
            &request(
                ReviewReadinessStateV1::Draft,
                ReviewReadinessStateV1::Ready,
                vec![passed("ci")],
            ),
        );
        assert_eq!(outcome.currentness, expected);
        assert!(
            !outcome.transition.permitted,
            "{} must never restore ready",
            expected.label()
        );
    }
}

#[test]
fn review_readiness_transition_permits_blocked_ready_to_draft_demotion() {
    let outcome = evaluate_review_disposition(
        &blocking_disposition(),
        &live(),
        &request(
            ReviewReadinessStateV1::Ready,
            ReviewReadinessStateV1::Draft,
            vec![],
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::ReviewBlocked);
    assert!(
        outcome.transition.permitted,
        "ready -> draft is the control action a blocked review requires"
    );
}

#[test]
fn review_readiness_transition_permits_current_clean_with_terminal_checks() {
    let outcome = evaluate_review_disposition(
        &disposition_with(ReviewCurrentnessV1::ReviewClean),
        &live(),
        &request(
            ReviewReadinessStateV1::Draft,
            ReviewReadinessStateV1::Ready,
            vec![passed("ci"), passed("guard")],
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::ReviewClean);
    assert!(outcome.transition.permitted);
    assert!(
        outcome
            .transition
            .reasons
            .iter()
            .any(|reason| reason.contains("every required check is terminal-passed"))
    );
}

#[test]
fn review_readiness_transition_demands_terminal_passed_outcomes() {
    for outcome in [
        CampaignCheckOutcomeV1::Failed,
        CampaignCheckOutcomeV1::Skipped,
        CampaignCheckOutcomeV1::Cancelled,
        CampaignCheckOutcomeV1::Nonterminal,
        CampaignCheckOutcomeV1::Unknown,
    ] {
        let evaluation = evaluate_review_disposition(
            &disposition_with(ReviewCurrentnessV1::ReviewClean),
            &live(),
            &request(
                ReviewReadinessStateV1::Draft,
                ReviewReadinessStateV1::Ready,
                vec![
                    passed("ci"),
                    ReviewCheckObservationV1 {
                        name: "guard".to_string(),
                        outcome,
                    },
                ],
            ),
        );
        assert!(
            !evaluation.transition.permitted,
            "a {outcome:?} check never permits ready"
        );
        assert!(
            evaluation
                .transition
                .reasons
                .iter()
                .any(|reason| reason.contains("required check 'guard' is"))
        );
    }
}

#[test]
fn review_readiness_transition_treats_same_state_requests_as_noops() {
    for state in [ReviewReadinessStateV1::Ready, ReviewReadinessStateV1::Draft] {
        let outcome = evaluate_review_disposition(
            &blocking_disposition(),
            &live(),
            &request(state, state, vec![]),
        );
        assert!(outcome.transition.permitted);
        assert!(
            outcome
                .transition
                .reasons
                .iter()
                .any(|reason| reason.contains("already in the requested state"))
        );
    }
}

#[test]
fn review_readiness_transition_permits_review_only_ready_without_declared_checks() {
    let outcome = evaluate_review_disposition(
        &disposition_with(ReviewCurrentnessV1::ReviewClean),
        &live(),
        &request(
            ReviewReadinessStateV1::Draft,
            ReviewReadinessStateV1::Ready,
            Vec::new(),
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::ReviewClean);
    assert!(outcome.transition.permitted);
}

#[test]
fn review_readiness_transition_rejects_a_required_check_with_an_empty_name() {
    let outcome = evaluate_review_disposition(
        &disposition_with(ReviewCurrentnessV1::ReviewClean),
        &live(),
        &request(
            ReviewReadinessStateV1::Draft,
            ReviewReadinessStateV1::Ready,
            vec![ReviewCheckObservationV1 {
                name: String::new(),
                outcome: CampaignCheckOutcomeV1::Passed,
            }],
        ),
    );
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
    assert!(
        outcome
            .currentness_reasons
            .iter()
            .any(|reason| reason.contains("empty name"))
    );
}
