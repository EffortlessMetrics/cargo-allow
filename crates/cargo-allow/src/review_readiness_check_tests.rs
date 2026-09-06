//! Projection-contract tests for the #3844 review-readiness check.

use allow_report::{
    CampaignCheckOutcomeV1, CampaignEvidenceClassV1, IndependentReviewPostureV1,
    REVIEW_READINESS_CHECK_CONTEXT, ReviewActorClassV1, ReviewCheckObservationV1,
    ReviewCurrentnessV1, ReviewDispositionV1, ReviewFindingSeverityV1, ReviewFindingV1,
    ReviewLiveSourceV1, ReviewReadinessBindingV1, ReviewReadinessConclusionV1,
    ReviewReadinessDispositionInputV1, ReviewReadinessDraftStateV1, ReviewReadinessEventV1,
    ReviewReadinessObservationV1, ReviewReadinessProjectionInputV1, ReviewReadinessStateV1,
    evaluate_review_readiness_projection, parse_review_disposition_bytes,
    render_review_readiness_human, render_review_readiness_json, review_semantic_identity,
};

fn clean_disposition() -> ReviewDispositionV1 {
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
        findings: vec![ReviewFindingV1 {
            id: "ADV-001".to_string(),
            severity: ReviewFindingSeverityV1::Advisory,
            owned_seam: "docs".to_string(),
            source_path: "docs/x.md".to_string(),
            source_line: Some(3),
            repair_route: "doc follow-up".to_string(),
            claim_boundary: "advisory only".to_string(),
        }],
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

fn input(
    disposition: ReviewReadinessDispositionInputV1,
    draft_state: ReviewReadinessDraftStateV1,
    event: ReviewReadinessEventV1,
    live_source: ReviewLiveSourceV1,
) -> ReviewReadinessProjectionInputV1 {
    input_with_delta(disposition, draft_state, event, live_source, Vec::new())
}

fn input_with_delta(
    disposition: ReviewReadinessDispositionInputV1,
    draft_state: ReviewReadinessDraftStateV1,
    event: ReviewReadinessEventV1,
    live_source: ReviewLiveSourceV1,
    head_delta_paths: Vec<String>,
) -> ReviewReadinessProjectionInputV1 {
    ReviewReadinessProjectionInputV1 {
        disposition,
        live: live_source,
        draft_state,
        event,
        prior_observation: None,
        head_delta_paths,
    }
}

#[test]
fn review_readiness_check_projects_a_current_clean_review_to_success() {
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Draft,
        ReviewReadinessEventV1::ReadyForReview,
        live(),
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Success);
    assert_eq!(projection.required_posture, ReviewReadinessStateV1::Ready);
    assert_eq!(projection.check_context, REVIEW_READINESS_CHECK_CONTEXT);
    assert!(!projection.stale_green_invalidated);
}

#[test]
fn review_readiness_check_fails_a_blocked_review_on_a_ready_pr() {
    let mut blocked = clean_disposition();
    blocked.claimed_verdict = ReviewCurrentnessV1::ReviewBlocked;
    blocked.findings = vec![ReviewFindingV1 {
        id: "BLK-001".to_string(),
        severity: ReviewFindingSeverityV1::Blocking,
        owned_seam: "crates/allow-match".to_string(),
        source_path: "crates/allow-match/src/lib.rs".to_string(),
        source_line: Some(40),
        repair_route: "repair on the same head".to_string(),
        claim_boundary: "blocking".to_string(),
    }];
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(blocked)),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        live(),
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
    assert_eq!(projection.required_posture, ReviewReadinessStateV1::Draft);
    assert!(
        projection
            .conclusion_reasons
            .iter()
            .any(|reason| reason.contains("never overrides a blocked review"))
    );
}

#[test]
fn review_readiness_check_treats_ci_as_a_separate_required_input() {
    // The projection input has no CI surface at all: aggregate check
    // state is consumed by the closeout verifier (#3845) and the live
    // controls (#2284), never by this check. A hostile input carrying
    // a checks array is rejected by the bounded schema.
    let hostile = serde_json::json!({
        "disposition": { "present": null },
        "checks": [{ "name": "ci", "conclusion": "success" }],
        "live": live(),
        "draft_state": "ready",
        "event": "synchronize",
        "prior_observation": null
    });
    let parse: Result<ReviewReadinessProjectionInputV1, _> = serde_json::from_value(hostile);
    assert!(
        parse.is_err(),
        "CI aggregate state cannot enter the readiness projection"
    );

    // And a blocked review stays a failure no matter what CI reports
    // elsewhere: the conclusion is computed from structured review
    // state only.
    let mut blocked = clean_disposition();
    blocked.claimed_verdict = ReviewCurrentnessV1::ReviewBlocked;
    blocked.findings = vec![ReviewFindingV1 {
        id: "BLK-002".to_string(),
        severity: ReviewFindingSeverityV1::Blocking,
        owned_seam: "seam".to_string(),
        source_path: "path.rs".to_string(),
        source_line: Some(1),
        repair_route: "repair".to_string(),
        claim_boundary: "blocking".to_string(),
    }];
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(blocked)),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Opened,
        live(),
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
}

#[test]
fn review_readiness_check_never_synthesizes_clean_from_missing_or_draft_state() {
    // A draft PR with no disposition is neutral, never clean.
    let draft_projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Missing,
        ReviewReadinessDraftStateV1::Draft,
        ReviewReadinessEventV1::Opened,
        live(),
    ));
    assert_eq!(
        draft_projection.conclusion,
        ReviewReadinessConclusionV1::Neutral
    );
    assert_eq!(
        draft_projection.required_posture,
        ReviewReadinessStateV1::Draft
    );

    // A ready PR with no disposition is still not clean.
    let ready_projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Missing,
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::ReadyForReview,
        live(),
    ));
    assert_eq!(
        ready_projection.conclusion,
        ReviewReadinessConclusionV1::Neutral
    );
    assert_eq!(
        ready_projection.required_posture,
        ReviewReadinessStateV1::Draft
    );
    assert!(
        ready_projection
            .conclusion_reasons
            .iter()
            .any(|reason| reason.contains("readiness is not proven"))
    );
}

#[test]
fn review_readiness_check_fails_malformed_and_prose_inputs() {
    let malformed = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Malformed {
            reason: "disposition parse: unknown field `comment`".to_string(),
        },
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        live(),
    ));
    assert_eq!(malformed.conclusion, ReviewReadinessConclusionV1::Failure);

    // Prose cannot enter as a disposition: the bounded adapter rejects
    // comment-shaped records outright.
    let prose = parse_review_disposition_bytes(br#"{"comment": "LGTM clean ship it"}"#);
    assert!(prose.is_err());
}

#[test]
fn review_readiness_check_fails_a_quota_limited_or_unavailable_review() {
    let mut not_proven = clean_disposition();
    not_proven.independent_review = IndependentReviewPostureV1::NotProven {
        reason: "reviewer quota exhausted".to_string(),
    };
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(not_proven)),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::ReadyForReview,
        live(),
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);

    let mut unavailable = clean_disposition();
    unavailable.actor_class = ReviewActorClassV1::Unavailable;
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(unavailable)),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::ReadyForReview,
        live(),
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
}

#[test]
fn review_readiness_check_rejects_wrong_pr_head_and_base_dispositions() {
    let mut wrong_pr = clean_disposition();
    wrong_pr.pr_number = 999;
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(wrong_pr)),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        live(),
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
    assert!(
        projection
            .conclusion_reasons
            .iter()
            .any(|reason| reason.contains("stale"))
    );

    let mut moved_head = live();
    moved_head.head_sha = "cccc".to_string();
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        moved_head,
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);

    let mut moved_base = live();
    moved_base.base_sha = "dddd".to_string();
    moved_base.merge_base = "dddd".to_string();
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        moved_base,
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
}

#[test]
fn review_readiness_check_binds_the_result_to_the_exact_live_pair() {
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Draft,
        ReviewReadinessEventV1::Opened,
        live(),
    ));
    assert_eq!(
        projection.binding,
        ReviewReadinessBindingV1 {
            repository: "owner/repo".to_string(),
            pr_number: 4146,
            base_ref: "main".to_string(),
            base_sha: "aaaa".to_string(),
            head_ref: "test/review-readiness-check".to_string(),
            head_sha: "bbbb".to_string(),
            merge_base: "aaaa".to_string(),
            diff_digest: "sha256:v1:digest".to_string(),
            disposition_identity: review_semantic_identity(&clean_disposition()),
        }
    );
}

#[test]
fn review_readiness_check_views_derive_from_one_projection() {
    let projection = evaluate_review_readiness_projection(&input(
        ReviewReadinessDispositionInputV1::Missing,
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::ReadyForReview,
        live(),
    ));
    let json = render_review_readiness_json(&projection).expect("serialization succeeds");
    let roundtrip: allow_report::ReviewReadinessProjectionV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, projection);
    let human = render_review_readiness_human(&projection);
    assert!(human.contains("conclusion: neutral"));
    assert!(human.contains("required posture: draft"));
    assert!(human.contains("review-readiness"));
}

#[test]
fn review_readiness_check_reports_nonterminal_check_observations_as_separate() {
    // The readiness check never embeds CI outcomes; the observation
    // vocabulary stays owned by the closeout contract. A nonterminal
    // observation cannot mutate the review conclusion because it is
    // not an input here: prove the type boundary by round-tripping the
    // check observation vocabulary used by #3845 consumers.
    let observation = ReviewCheckObservationV1 {
        name: "ci".to_string(),
        outcome: CampaignCheckOutcomeV1::Nonterminal,
    };
    let bytes = serde_json::to_vec(&observation).expect("fixture serialization");
    let parsed: ReviewCheckObservationV1 =
        serde_json::from_slice(&bytes).expect("observation parses");
    assert_eq!(parsed.outcome, CampaignCheckOutcomeV1::Nonterminal);
}

#[test]
fn review_readiness_check_admits_a_ledger_only_head_delta() {
    // A disposition committed inside its own PR moves the head it
    // binds. The projection admits that movement only when the entire
    // delta is review-disposition records.
    let mut one_commit_ahead = live();
    one_commit_ahead.head_sha = "cccc".to_string();
    let mut ledger_delta = one_commit_ahead.clone();
    ledger_delta.diff_digest = "sha256:v1:with-ledger".to_string();
    let projection = evaluate_review_readiness_projection(&input_with_delta(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        ledger_delta,
        vec![
            ".allow/review-dispositions/4146-bbbb.json".to_string(),
            ".allow/review-dispositions/index.json".to_string(),
        ],
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Success);
    assert!(projection.head_ledger_bootstrap);
    assert!(
        projection
            .conclusion_reasons
            .iter()
            .any(|reason| reason.contains("review-ledger bootstrap"))
    );

    // Any non-ledger file in the delta is ordinary staleness.
    let projection = evaluate_review_readiness_projection(&input_with_delta(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::Synchronize,
        one_commit_ahead,
        vec![
            ".allow/review-dispositions/4146-bbbb.json".to_string(),
            "crates/allow-match/src/lib.rs".to_string(),
        ],
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
    assert!(!projection.head_ledger_bootstrap);
}

#[test]
fn review_readiness_check_never_waives_base_movement_through_the_ledger() {
    let mut moved_base = live();
    moved_base.base_sha = "dddd".to_string();
    moved_base.merge_base = "dddd".to_string();
    let projection = evaluate_review_readiness_projection(&input_with_delta(
        ReviewReadinessDispositionInputV1::Present(Box::new(clean_disposition())),
        ReviewReadinessDraftStateV1::Ready,
        ReviewReadinessEventV1::BaseMoved,
        moved_base,
        vec![".allow/review-dispositions/4146-bbbb.json".to_string()],
    ));
    assert_eq!(projection.conclusion, ReviewReadinessConclusionV1::Failure);
}

#[test]
fn review_readiness_check_prior_observation_parses_from_the_checked_schema() {
    let prior = ReviewReadinessObservationV1 {
        conclusion: ReviewReadinessConclusionV1::Success,
        binding: ReviewReadinessBindingV1 {
            repository: "owner/repo".to_string(),
            pr_number: 4146,
            base_ref: "main".to_string(),
            base_sha: "aaaa".to_string(),
            head_ref: "test/review-readiness-check".to_string(),
            head_sha: "bbbb".to_string(),
            merge_base: "aaaa".to_string(),
            diff_digest: "sha256:v1:digest".to_string(),
            disposition_identity: "fnv1a64:0123456789abcdef".to_string(),
        },
    };
    let bytes = serde_json::to_vec(&prior).expect("fixture serialization");
    let parsed: ReviewReadinessObservationV1 =
        serde_json::from_slice(&bytes).expect("observation parses");
    assert_eq!(parsed, prior);
}
