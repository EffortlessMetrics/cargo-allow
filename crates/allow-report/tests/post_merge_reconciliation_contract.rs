use allow_report::{
    LandedEffectStatusV1, LandedEffectV1, NextFrontierV1, PostMergeReconciliationRequestV1,
    PostMergeReconciliationResultV1, ReconciliationDispositionV1,
};

fn request() -> PostMergeReconciliationRequestV1 {
    PostMergeReconciliationRequestV1 {
        reconciliation_id: "reconcile-001".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        target_branch: "main".to_string(),
        merge_commit_sha: "merge-001".to_string(),
        merge_tree_sha: "tree-001".to_string(),
        integration_receipt_merge_commit_sha: "merge-001".to_string(),
        integration_receipt_merge_tree_sha: "tree-001".to_string(),
        current_main_commit_sha: "merge-001".to_string(),
        current_main_tree_sha: "tree-001".to_string(),
        integration_receipt_id: "integration-001".to_string(),
        claim_ref: "claim-001".to_string(),
        expected_effects: vec![LandedEffectV1 {
            effect_id: "effect-001".to_string(),
            status: LandedEffectStatusV1::Present,
        }],
        premerge_evidence_current: true,
        post_merge_obligations_complete: true,
        repository_decision_required: false,
        external_action_required: false,
        contradiction: false,
    }
}

#[test]
fn exact_landed_state_reconciles_without_mutation_authority() {
    let result = PostMergeReconciliationResultV1::evaluate(request());
    assert_eq!(result.disposition, ReconciliationDispositionV1::Reconciled);
    assert_eq!(result.next_frontier, NextFrontierV1::Complete);
    assert_eq!(result.current_main_commit_sha, "merge-001");
    assert!(
        result
            .limitations
            .iter()
            .any(|item| item == "does_not_mutate_source_or_external_authority")
    );
}

#[test]
fn moved_main_is_stale_even_when_the_merge_was_exact() {
    let mut input = request();
    input.current_main_commit_sha = "later-002".to_string();
    input.current_main_tree_sha = "tree-002".to_string();
    let result = PostMergeReconciliationResultV1::evaluate(input);
    assert_eq!(result.disposition, ReconciliationDispositionV1::Stale);
    assert_eq!(result.next_frontier, NextFrontierV1::RefreshCurrentIntent);
}

#[test]
fn receipt_subject_mismatch_is_stale() {
    let mut input = request();
    input.integration_receipt_merge_tree_sha = "different-tree".to_string();
    let result = PostMergeReconciliationResultV1::evaluate(input);
    assert_eq!(result.disposition, ReconciliationDispositionV1::Stale);
}

#[test]
fn partial_and_missing_effects_cannot_complete_the_claim() {
    let mut input = request();
    input.expected_effects.push(LandedEffectV1 {
        effect_id: "effect-002".to_string(),
        status: LandedEffectStatusV1::Missing,
    });
    let result = PostMergeReconciliationResultV1::evaluate(input);
    assert_eq!(
        result.disposition,
        ReconciliationDispositionV1::PartiallyReconciled
    );
    assert_eq!(
        result.next_frontier,
        NextFrontierV1::OpenOrLinkFollowUpClaim
    );
}

#[test]
fn unknown_facts_are_not_a_clean_result() {
    let mut input = request();
    input.expected_effects = vec![LandedEffectV1 {
        effect_id: "effect-001".to_string(),
        status: LandedEffectStatusV1::Unknown,
    }];
    let result = PostMergeReconciliationResultV1::evaluate(input);
    assert_eq!(result.disposition, ReconciliationDispositionV1::NotProven);
    assert_eq!(result.next_frontier, NextFrontierV1::RunPostMergeProof);
}

#[test]
fn malformed_and_duplicate_inputs_fail_closed() {
    let mut malformed = request();
    malformed.claim_ref = "bad\nclaim".to_string();
    assert_eq!(
        PostMergeReconciliationResultV1::evaluate(malformed).disposition,
        ReconciliationDispositionV1::InstrumentFailure
    );

    let mut duplicate = request();
    duplicate.expected_effects.push(LandedEffectV1 {
        effect_id: "effect-001".to_string(),
        status: LandedEffectStatusV1::Present,
    });
    assert_eq!(
        PostMergeReconciliationResultV1::evaluate(duplicate).disposition,
        ReconciliationDispositionV1::InstrumentFailure
    );
}

#[test]
fn result_round_trips_as_versioned_machine_contract() -> Result<(), serde_json::Error> {
    let result = PostMergeReconciliationResultV1::evaluate(request());
    let encoded = serde_json::to_string(&result)?;
    let decoded: PostMergeReconciliationResultV1 = serde_json::from_str(&encoded)?;
    assert_eq!(decoded, result);
    assert_eq!(
        decoded.schema_id,
        PostMergeReconciliationResultV1::CURRENT_SCHEMA_ID
    );
    Ok(())
}
