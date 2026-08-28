//! Read-only post-merge reconciliation over supplied landed-state facts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReconciliationDispositionV1 {
    Reconciled,
    ReconciliationPending,
    PartiallyReconciled,
    Stale,
    Contradictory,
    NeedsRepositoryDecision,
    ExternalActionRequired,
    NotProven,
    InstrumentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NextFrontierV1 {
    Complete,
    RefreshCurrentIntent,
    RunPostMergeProof,
    AmendRepositoryAuthority,
    ReconcileDocumentationOrSupport,
    OpenOrLinkFollowUpClaim,
    ResolveContradiction,
    PerformExternalAction,
    UpdateIssueOrController,
    ReviewResidue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandedEffectStatusV1 {
    Present,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedEffectV1 {
    pub effect_id: String,
    pub status: LandedEffectStatusV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMergeReconciliationRequestV1 {
    pub reconciliation_id: String,
    pub repository: String,
    pub target_branch: String,
    pub merge_commit_sha: String,
    pub merge_tree_sha: String,
    pub integration_receipt_merge_commit_sha: String,
    pub integration_receipt_merge_tree_sha: String,
    pub current_main_commit_sha: String,
    pub current_main_tree_sha: String,
    pub integration_receipt_id: String,
    pub claim_ref: String,
    pub expected_effects: Vec<LandedEffectV1>,
    pub premerge_evidence_current: bool,
    pub post_merge_obligations_complete: bool,
    pub repository_decision_required: bool,
    pub external_action_required: bool,
    pub contradiction: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMergeReconciliationResultV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub reconciliation_id: String,
    pub repository: String,
    pub target_branch: String,
    pub current_main_commit_sha: String,
    pub current_main_tree_sha: String,
    pub integration_receipt_id: String,
    pub disposition: ReconciliationDispositionV1,
    pub next_frontier: NextFrontierV1,
    pub landed_effects: Vec<LandedEffectV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: Vec<String>,
}

impl PostMergeReconciliationResultV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = "cargo-allow.post-merge-reconciliation.v1";
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn evaluate(input: PostMergeReconciliationRequestV1) -> Self {
        let invalid = [
            &input.reconciliation_id,
            &input.repository,
            &input.target_branch,
            &input.merge_commit_sha,
            &input.merge_tree_sha,
            &input.integration_receipt_merge_commit_sha,
            &input.integration_receipt_merge_tree_sha,
            &input.current_main_commit_sha,
            &input.current_main_tree_sha,
            &input.integration_receipt_id,
            &input.claim_ref,
        ]
        .iter()
        .any(|value| value.is_empty() || has_forbidden_token(value));

        let disposition = if invalid
            || input.expected_effects.is_empty()
            || has_duplicate_effect_ids(&input.expected_effects)
        {
            ReconciliationDispositionV1::InstrumentFailure
        } else if input.merge_commit_sha != input.current_main_commit_sha
            || input.merge_tree_sha != input.current_main_tree_sha
            || input.merge_commit_sha != input.integration_receipt_merge_commit_sha
            || input.merge_tree_sha != input.integration_receipt_merge_tree_sha
        {
            ReconciliationDispositionV1::Stale
        } else if input.contradiction {
            ReconciliationDispositionV1::Contradictory
        } else if input.repository_decision_required {
            ReconciliationDispositionV1::NeedsRepositoryDecision
        } else if input.external_action_required {
            ReconciliationDispositionV1::ExternalActionRequired
        } else if input
            .expected_effects
            .iter()
            .any(|effect| effect.status == LandedEffectStatusV1::Unknown)
            || !input.premerge_evidence_current
        {
            ReconciliationDispositionV1::NotProven
        } else if input
            .expected_effects
            .iter()
            .any(|effect| effect.status == LandedEffectStatusV1::Missing)
        {
            if input
                .expected_effects
                .iter()
                .any(|effect| effect.status == LandedEffectStatusV1::Present)
            {
                ReconciliationDispositionV1::PartiallyReconciled
            } else {
                ReconciliationDispositionV1::Contradictory
            }
        } else if !input.post_merge_obligations_complete {
            ReconciliationDispositionV1::ReconciliationPending
        } else {
            ReconciliationDispositionV1::Reconciled
        };

        let next_frontier = next_frontier_for(disposition);
        let limitations = vec![
            "does_not_mutate_source_or_external_authority".to_string(),
            "does_not_execute_follow_up_operations".to_string(),
        ];
        Self {
            schema_id: Self::CURRENT_SCHEMA_ID.to_string(),
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            reconciliation_id: input.reconciliation_id,
            repository: input.repository,
            target_branch: input.target_branch,
            current_main_commit_sha: input.current_main_commit_sha,
            current_main_tree_sha: input.current_main_tree_sha,
            integration_receipt_id: input.integration_receipt_id,
            disposition,
            next_frontier,
            landed_effects: input.expected_effects,
            limitations,
            claim_boundary: vec![
                "reconciles_supplied_current_main_facts".to_string(),
                "preserves_premerge_evidence_subject_boundary".to_string(),
                "reports_follow_up_without_mutation".to_string(),
            ],
        }
    }
}

fn has_duplicate_effect_ids(effects: &[LandedEffectV1]) -> bool {
    effects.iter().enumerate().any(|(index, effect)| {
        effects[..index]
            .iter()
            .any(|previous| previous.effect_id == effect.effect_id)
    })
}

fn has_forbidden_token(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\0' | '\n' | '\r' | ';' | '|' | '`' | '$'))
}

fn next_frontier_for(disposition: ReconciliationDispositionV1) -> NextFrontierV1 {
    match disposition {
        ReconciliationDispositionV1::Reconciled => NextFrontierV1::Complete,
        ReconciliationDispositionV1::ReconciliationPending => NextFrontierV1::RunPostMergeProof,
        ReconciliationDispositionV1::PartiallyReconciled => NextFrontierV1::OpenOrLinkFollowUpClaim,
        ReconciliationDispositionV1::Stale => NextFrontierV1::RefreshCurrentIntent,
        ReconciliationDispositionV1::Contradictory => NextFrontierV1::ResolveContradiction,
        ReconciliationDispositionV1::NeedsRepositoryDecision => {
            NextFrontierV1::AmendRepositoryAuthority
        }
        ReconciliationDispositionV1::ExternalActionRequired => {
            NextFrontierV1::PerformExternalAction
        }
        ReconciliationDispositionV1::NotProven => NextFrontierV1::RunPostMergeProof,
        ReconciliationDispositionV1::InstrumentFailure => NextFrontierV1::ReviewResidue,
    }
}
