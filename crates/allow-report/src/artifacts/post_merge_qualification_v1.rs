//! Post-merge tree equivalence and candidate requalification authority.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeMethodV1 {
    Squash,
    Rebase,
    MergeCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostMergeEquivalenceVerdictV1 {
    EquivalentTree,
    EquivalentSelectedBytes,
    RequalificationRequired,
    Stale,
    Mismatch,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewedContextV1 {
    pub base_sha: String,
    pub head_sha: String,
    pub merge_base_sha: String,
    pub tree_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergedStateV1 {
    pub pr_number: u64,
    pub merge_commit_sha: String,
    pub merge_tree_sha: String,
    pub current_main_commit_sha: String,
    pub current_main_tree_sha: String,
    pub merge_method: MergeMethodV1,
    pub merge_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostMergeQualificationInitV1 {
    pub qualification_id: String,
    pub reviewed: ReviewedContextV1,
    pub merged: MergedStateV1,
    pub changed_files: Vec<String>,
    pub semantic_owners: Vec<String>,
    pub premerge_evidence_digest: String,
    pub preserved_evidence_nodes: Vec<String>,
    pub invalidated_evidence_nodes: Vec<String>,
    pub required_rerun_set: Vec<String>,
    pub created_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowPostMergeQualificationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub qualification_id: String,
    pub reviewed: ReviewedContextV1,
    pub merged: MergedStateV1,
    pub changed_files: Vec<String>,
    pub semantic_owners: Vec<String>,
    pub premerge_evidence_digest: String,
    pub preserved_evidence_nodes: Vec<String>,
    pub invalidated_evidence_nodes: Vec<String>,
    pub required_rerun_set: Vec<String>,
    pub created_at_utc: String,
    pub claim_boundary: Vec<String>,
    pub limitations: Vec<String>,
}

impl CargoAllowPostMergeQualificationV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = "cargo-allow.post-merge-qualification.v1";
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn new(init: PostMergeQualificationInitV1) -> Self {
        Self {
            schema_id: Self::CURRENT_SCHEMA_ID.to_string(),
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            qualification_id: init.qualification_id,
            reviewed: init.reviewed,
            merged: init.merged,
            changed_files: init.changed_files,
            semantic_owners: init.semantic_owners,
            premerge_evidence_digest: init.premerge_evidence_digest,
            preserved_evidence_nodes: init.preserved_evidence_nodes,
            invalidated_evidence_nodes: init.invalidated_evidence_nodes,
            required_rerun_set: init.required_rerun_set,
            created_at_utc: init.created_at_utc,
            claim_boundary: vec![
                "exact_tree_sha_equivalence_evaluation".to_string(),
                "transitive_evidence_invalidation_tracking".to_string(),
                "bound_merged_main_continuity".to_string(),
                "no_unauthorized_state_mutation".to_string(),
            ],
            limitations: vec![
                "does_not_execute_ci_reruns_directly".to_string(),
                "does_not_replace_formal_release_authorization".to_string(),
            ],
        }
    }

    pub fn evaluate_verdict(&self) -> PostMergeEquivalenceVerdictV1 {
        // Schema and structural integrity
        if self.schema_id != Self::CURRENT_SCHEMA_ID
            || self.schema_version != Self::CURRENT_SCHEMA_VERSION
            || self.qualification_id.is_empty()
            || self.reviewed.tree_sha.is_empty()
            || self.merged.merge_tree_sha.is_empty()
        {
            return PostMergeEquivalenceVerdictV1::InstrumentFailure;
        }

        if has_forbidden_tokens(&self.qualification_id)
            || has_forbidden_tokens(&self.reviewed.base_sha)
            || has_forbidden_tokens(&self.reviewed.head_sha)
            || has_forbidden_tokens(&self.merged.merge_commit_sha)
        {
            return PostMergeEquivalenceVerdictV1::InstrumentFailure;
        }

        // Check if main has moved past the merge commit
        if self.merged.current_main_commit_sha != self.merged.merge_commit_sha
            || self.merged.current_main_tree_sha != self.merged.merge_tree_sha
        {
            return PostMergeEquivalenceVerdictV1::Stale;
        }

        // Check merge parents consistency
        if self.merged.merge_parents.is_empty() {
            return PostMergeEquivalenceVerdictV1::Mismatch;
        }

        // If invalidated nodes exist or required reruns exist, requalification is mandatory
        if !self.invalidated_evidence_nodes.is_empty() || !self.required_rerun_set.is_empty() {
            return PostMergeEquivalenceVerdictV1::RequalificationRequired;
        }

        // Exact tree match
        if self.reviewed.tree_sha == self.merged.merge_tree_sha {
            return PostMergeEquivalenceVerdictV1::EquivalentTree;
        }

        // Selected package bytes match with documented differences
        if self
            .changed_files
            .iter()
            .all(|f| f.ends_with(".md") || f.starts_with(".changes/"))
        {
            return PostMergeEquivalenceVerdictV1::EquivalentSelectedBytes;
        }

        PostMergeEquivalenceVerdictV1::RequalificationRequired
    }
}

fn has_forbidden_tokens(text: &str) -> bool {
    text.chars().any(|c| {
        c == '\0' || c == '\n' || c == '\r' || c == ';' || c == '|' || c == '`' || c == '$'
    })
}
