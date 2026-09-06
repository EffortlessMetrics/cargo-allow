//! Currentness-law tests for the #3843 review-disposition model:
//! head, base, merge-base, full-diff, protocol, scope, and identity
//! movement stale a prior review deterministically.

use allow_report::{
    CampaignCheckOutcomeV1, CampaignEvidenceClassV1, IndependentReviewPostureV1,
    ReviewActorClassV1, ReviewCheckObservationV1, ReviewCurrentnessV1, ReviewDispositionV1,
    ReviewFindingSeverityV1, ReviewFindingV1, ReviewLiveSourceV1, ReviewReadinessStateV1,
    ReviewRequiredCiV1, ReviewTransitionRequestV1, evaluate_review_disposition,
};

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
            owned_seam: "crates/allow-match".to_string(),
            source_path: "crates/allow-match/src/lib.rs".to_string(),
            source_line: Some(12),
            repair_route: "follow-up lane".to_string(),
            claim_boundary: "advisory observation".to_string(),
        }],
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

fn ready_request() -> ReviewTransitionRequestV1 {
    ReviewTransitionRequestV1 {
        current_state: ReviewReadinessStateV1::Draft,
        target_state: ReviewReadinessStateV1::Ready,
        required_checks: vec![ReviewCheckObservationV1 {
            name: "ci".to_string(),
            outcome: CampaignCheckOutcomeV1::Passed,
        }],
    }
}

#[test]
fn review_disposition_currentness_stales_when_the_head_moves_after_review() {
    let current = evaluate_review_disposition(&disposition(), &live(), &ready_request());
    assert_eq!(current.currentness, ReviewCurrentnessV1::ReviewClean);

    let mut moved = live();
    moved.head_sha = "cccc".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &moved, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "head_sha")
    );
}

#[test]
fn review_disposition_currentness_stales_when_the_base_or_merge_base_moves() {
    let mut moved_base = live();
    moved_base.base_sha = "cccc".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &moved_base, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "base_sha")
    );
    assert!(
        !outcome.transition.permitted,
        "a review of an old base is not sufficient after base movement"
    );

    let mut moved_merge_base = live();
    moved_merge_base.merge_base = "dddd".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &moved_merge_base, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "merge_base")
    );
}

#[test]
fn review_disposition_currentness_stales_when_the_full_diff_differs_under_equal_labels() {
    let mut relabeled = live();
    relabeled.diff_digest = "sha256:v1:other-content".to_string();
    assert_eq!(relabeled.head_ref, disposition().head_ref);
    assert_eq!(relabeled.head_sha, disposition().head_sha);
    let outcome = evaluate_review_disposition(&disposition(), &relabeled, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "reviewed_diff_digest")
    );
}

#[test]
fn review_disposition_currentness_stales_on_protocol_generation_change() {
    let mut regenerated = live();
    regenerated.review_protocol = "review-current-head-gen2".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &regenerated, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "review_protocol")
    );
}

#[test]
fn review_disposition_currentness_stales_on_reviewer_scope_change() {
    let mut widened = live();
    widened.scope_claim_boundary = "issue:3843 issue:9999".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &widened, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "review_scope")
    );
}

#[test]
fn review_disposition_currentness_stales_on_repository_or_pr_identity_change() {
    let mut renamed = live();
    renamed.repository = "owner/other".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &renamed, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "repository")
    );

    let mut renumbered = live();
    renumbered.pr_number = 5000;
    let outcome = evaluate_review_disposition(&disposition(), &renumbered, &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Stale);
    assert!(
        outcome
            .stale_dimensions
            .iter()
            .any(|dimension| dimension == "pr_number")
    );
}

#[test]
fn review_disposition_currentness_lets_structure_win_over_a_clean_claim() {
    let mut blocked = disposition();
    blocked.claimed_verdict = ReviewCurrentnessV1::ReviewClean;
    blocked.findings = vec![ReviewFindingV1 {
        id: "BLK-001".to_string(),
        severity: ReviewFindingSeverityV1::Blocking,
        owned_seam: "crates/allow-match".to_string(),
        source_path: "crates/allow-match/src/lib.rs".to_string(),
        source_line: Some(40),
        repair_route: "repair lane on the same head".to_string(),
        claim_boundary: "blocking".to_string(),
    }];
    let outcome = evaluate_review_disposition(&blocked, &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::ReviewBlocked);
    assert!(
        outcome
            .blocking_finding_ids
            .iter()
            .any(|id| id == "BLK-001")
    );
    assert!(!outcome.transition.permitted);
}

#[test]
fn review_disposition_currentness_reports_moved_dimensions_in_a_fixed_order() {
    let mut all_moved = live();
    all_moved.repository = "owner/other".to_string();
    all_moved.pr_number = 5000;
    all_moved.base_sha = "cccc".to_string();
    all_moved.head_sha = "dddd".to_string();
    all_moved.merge_base = "eeee".to_string();
    all_moved.diff_digest = "sha256:v1:elsewhere".to_string();
    all_moved.review_protocol = "review-current-head-gen2".to_string();
    all_moved.scope_claim_boundary = "issue:0000".to_string();
    let outcome = evaluate_review_disposition(&disposition(), &all_moved, &ready_request());
    assert_eq!(
        outcome.stale_dimensions,
        vec![
            "repository".to_string(),
            "pr_number".to_string(),
            "base_sha".to_string(),
            "head_sha".to_string(),
            "merge_base".to_string(),
            "reviewed_diff_digest".to_string(),
            "review_protocol".to_string(),
            "review_scope".to_string(),
        ]
    );
}
