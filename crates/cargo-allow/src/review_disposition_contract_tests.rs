//! Contract tests for the #3843 review-disposition schema and model.

use allow_report::{
    CampaignEvidenceClassV1, IndependentReviewPostureV1, REVIEW_DISPOSITION_MAX_FINDINGS,
    REVIEW_DISPOSITION_MAX_THREADS, ReviewActorClassV1, ReviewCheckObservationV1,
    ReviewCurrentnessV1, ReviewDispositionOutcomeV1, ReviewDispositionV1, ReviewFindingSeverityV1,
    ReviewFindingV1, ReviewLiveSourceV1, ReviewReadinessStateV1, ReviewRequiredCiV1,
    ReviewTransitionRequestV1, evaluate_review_disposition, parse_review_disposition_bytes,
    render_review_disposition_human, render_review_disposition_json, review_semantic_identity,
};

fn finding(id: &str, severity: ReviewFindingSeverityV1) -> ReviewFindingV1 {
    ReviewFindingV1 {
        id: id.to_string(),
        severity,
        owned_seam: "crates/allow-match".to_string(),
        source_path: "crates/allow-match/src/lib.rs".to_string(),
        source_line: Some(12),
        repair_route: "follow-up lane".to_string(),
        claim_boundary: "advisory observation".to_string(),
    }
}

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
        findings: vec![finding("ADV-001", ReviewFindingSeverityV1::Advisory)],
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

fn ready_request() -> ReviewTransitionRequestV1 {
    ReviewTransitionRequestV1 {
        current_state: ReviewReadinessStateV1::Draft,
        target_state: ReviewReadinessStateV1::Ready,
        required_checks: vec![ReviewCheckObservationV1 {
            name: "ci".to_string(),
            outcome: allow_report::CampaignCheckOutcomeV1::Passed,
        }],
    }
}

#[test]
fn review_disposition_contract_parses_and_roundtrips_a_typed_disposition() {
    let bytes = serde_json::to_vec(&disposition()).expect("fixture serialization succeeds");
    let parsed = parse_review_disposition_bytes(&bytes).expect("a well-formed disposition parses");
    assert_eq!(parsed, disposition());
}

#[test]
fn review_disposition_contract_rejects_a_blocking_finding_without_stable_id_source_or_repair() {
    for empty_field in ["id", "source_path", "repair_route", "owned_seam"] {
        let mut malformed = disposition();
        malformed.findings = vec![finding("BLK-001", ReviewFindingSeverityV1::Blocking)];
        let first = malformed
            .findings
            .first_mut()
            .expect("the fixture retains one finding");
        match empty_field {
            "id" => first.id = String::new(),
            "source_path" => first.source_path = String::new(),
            "repair_route" => first.repair_route = String::new(),
            _ => first.owned_seam = String::new(),
        }
        let bytes = serde_json::to_vec(&malformed).expect("fixture serialization succeeds");
        let parse = parse_review_disposition_bytes(&bytes);
        assert!(parse.is_err(), "empty {empty_field} must fail the adapter");
        let outcome = evaluate_review_disposition(&malformed, &live(), &ready_request());
        assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
        assert!(
            outcome
                .currentness_reasons
                .iter()
                .any(|reason| reason.contains("blocking finding is missing"))
        );
    }
}

#[test]
fn review_disposition_contract_rejects_approval_count_and_comment_substitutes() {
    let text = serde_json::to_string(&disposition()).expect("fixture serialization succeeds");
    let poisoned = text.replace(
        "\"schema_version\":1,",
        "\"schema_version\":1,\"approvals\":12,",
    );
    assert_ne!(text, poisoned, "the fixture must actually be poisoned");
    let parse = parse_review_disposition_bytes(poisoned.as_bytes());
    assert!(
        parse.is_err(),
        "an approval count cannot synthesize a typed disposition"
    );
    let comment_only = b"{\"comment\": \"LGTM, looks good to me, ship it\", \"approvals\": 3}";
    assert!(parse_review_disposition_bytes(comment_only).is_err());
}

#[test]
fn review_disposition_contract_downgrades_unavailable_or_not_proven_clean_claims() {
    let mut unavailable = disposition();
    unavailable.actor_class = ReviewActorClassV1::Unavailable;
    let outcome = evaluate_review_disposition(&unavailable, &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Partial);
    assert!(
        outcome
            .currentness_reasons
            .iter()
            .any(|reason| reason.contains("not proven clean"))
    );

    let mut not_proven = disposition();
    not_proven.independent_review = IndependentReviewPostureV1::NotProven {
        reason: "reviewer quota exhausted".to_string(),
    };
    let outcome = evaluate_review_disposition(&not_proven, &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::Partial);
    assert!(
        outcome
            .currentness_reasons
            .iter()
            .any(|reason| reason.contains("not proven"))
    );
}

#[test]
fn review_disposition_contract_excludes_volatile_fields_from_semantic_identity() {
    let baseline = review_semantic_identity(&disposition());
    let mut volatile = disposition();
    volatile.reviewed_at_utc = "2026-09-05T12:34:56Z".to_string();
    volatile.reviewer_identity = "another-display-name".to_string();
    volatile.threads_inspected = vec!["thread-2".to_string(), "thread-3".to_string()];
    volatile.required_ci.observation_ref = "run:98765".to_string();
    volatile.evidence_class = CampaignEvidenceClassV1::Prose;
    assert_eq!(
        baseline,
        review_semantic_identity(&volatile),
        "volatile presentation fields must not change the semantic identity"
    );

    let mut moved_head = disposition();
    moved_head.head_sha = "cccc".to_string();
    assert_ne!(baseline, review_semantic_identity(&moved_head));

    let mut moved_finding = disposition();
    if let Some(finding) = moved_finding.findings.first_mut() {
        finding.source_line = Some(13);
    }
    assert_ne!(baseline, review_semantic_identity(&moved_finding));

    let mut moved_severity = disposition();
    if let Some(finding) = moved_severity.findings.first_mut() {
        finding.severity = ReviewFindingSeverityV1::Blocking;
    }
    assert_ne!(baseline, review_semantic_identity(&moved_severity));
}

#[test]
fn review_disposition_contract_is_pure_and_claims_no_mutation() {
    let outcome_one = evaluate_review_disposition(&disposition(), &live(), &ready_request());
    let outcome_two = evaluate_review_disposition(&disposition(), &live(), &ready_request());
    assert_eq!(outcome_one, outcome_two);
    assert!(
        outcome_one
            .claim_boundary
            .contains("does not mutate PR state, live settings, tags, packages, or release state")
    );
}

#[test]
fn review_disposition_contract_human_and_json_views_derive_from_one_result() {
    let outcome = evaluate_review_disposition(&disposition(), &live(), &ready_request());
    let json = render_review_disposition_json(&outcome).expect("serialization succeeds");
    let roundtrip: ReviewDispositionOutcomeV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, outcome);
    let human = render_review_disposition_human(&outcome);
    assert!(human.contains(&format!("currentness: {}", outcome.currentness.label())));
    assert!(human.contains("transition: draft -> ready: permitted"));
}

#[test]
fn review_disposition_contract_bounds_finding_and_thread_counts() {
    let mut overbound = disposition();
    overbound.findings = (0..=REVIEW_DISPOSITION_MAX_FINDINGS)
        .map(|index| finding(&format!("ADV-{index}"), ReviewFindingSeverityV1::Advisory))
        .collect();
    let bytes = serde_json::to_vec(&overbound).expect("fixture serialization succeeds");
    let parse = parse_review_disposition_bytes(&bytes);
    assert!(parse.is_err(), "the finding bound must fail closed");

    let mut at_bound = disposition();
    at_bound.findings = (0..REVIEW_DISPOSITION_MAX_FINDINGS)
        .map(|index| finding(&format!("ADV-{index}"), ReviewFindingSeverityV1::Advisory))
        .collect();
    let bytes = serde_json::to_vec(&at_bound).expect("fixture serialization succeeds");
    assert!(parse_review_disposition_bytes(&bytes).is_ok());

    let mut threads = disposition();
    threads.threads_inspected = (0..=REVIEW_DISPOSITION_MAX_THREADS)
        .map(|index| format!("thread-{index}"))
        .collect();
    let bytes = serde_json::to_vec(&threads).expect("fixture serialization succeeds");
    assert!(parse_review_disposition_bytes(&bytes).is_err());
}

#[test]
fn review_disposition_contract_retains_same_maintainer_process_evidence_honestly() {
    let outcome = evaluate_review_disposition(&disposition(), &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::ReviewClean);
    assert_eq!(outcome.actor_class, ReviewActorClassV1::SameMaintainer);
    let human = render_review_disposition_human(&outcome);
    assert!(human.contains("actor: same_maintainer"));
}

#[test]
fn review_disposition_contract_rejects_derived_only_claims() {
    for claimed in [
        ReviewCurrentnessV1::Stale,
        ReviewCurrentnessV1::InstrumentFailure,
    ] {
        let mut incoherent = disposition();
        incoherent.claimed_verdict = claimed;
        let outcome = evaluate_review_disposition(&incoherent, &live(), &ready_request());
        assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
        assert!(
            outcome
                .currentness_reasons
                .iter()
                .any(|reason| reason.contains("derived-only verdict"))
        );
    }
}

#[test]
fn review_disposition_contract_rejects_a_blocked_claim_without_blocking_findings() {
    let mut incoherent = disposition();
    incoherent.claimed_verdict = ReviewCurrentnessV1::ReviewBlocked;
    incoherent.findings = Vec::new();
    let outcome = evaluate_review_disposition(&incoherent, &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
    assert!(
        outcome
            .currentness_reasons
            .iter()
            .any(|reason| reason.contains("no blocking finding is retained"))
    );
}

#[test]
fn review_disposition_contract_rejects_an_unavailable_actor_with_proven_review() {
    let mut contradictory = disposition();
    contradictory.actor_class = ReviewActorClassV1::Unavailable;
    contradictory.independent_review = IndependentReviewPostureV1::Proven {
        reference: "review-record:1".to_string(),
    };
    let outcome = evaluate_review_disposition(&contradictory, &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
    assert!(
        outcome
            .currentness_reasons
            .iter()
            .any(|reason| reason.contains("unavailable but independent review"))
    );
}

#[test]
fn review_disposition_contract_requires_ci_ownership_or_an_observation_reference() {
    let mut orphan = disposition();
    orphan.required_ci = ReviewRequiredCiV1 {
        owner: String::new(),
        observation_ref: String::new(),
    };
    let outcome = evaluate_review_disposition(&orphan, &live(), &ready_request());
    assert_eq!(outcome.currentness, ReviewCurrentnessV1::InstrumentFailure);
    assert!(
        outcome
            .currentness_reasons
            .iter()
            .any(|reason| reason.contains("required_ci"))
    );
}
