//! Final typed preparation receipt for the candidate-preparation product
//! (#3834).
//!
//! Emitted after one successful exact-plan apply, this receipt reconciles
//! the applied source state against release identity, package topology,
//! support/channel source, governed-file posture, and post-apply
//! validation. `Complete` means the source candidate was prepared
//! coherently. It does not mean exact package bytes, final usability,
//! public provider state, authorization, or release are complete.

use serde::{Deserialize, Serialize};

pub const CANDIDATE_PREPARATION_RECEIPT_SCHEMA_V1: &str =
    "cargo-allow.candidate-preparation-receipt.v1";

pub const CANDIDATE_PREPARATION_RECEIPT_CLAIM_BOUNDARY_V1: &str = "Integrated source-candidate preparation across exact version/package projection, Changie/history, release documentation, support/channel source, governed files, and post-apply validation. It emits the source-preparation receipt consumed by final qualification; it does not qualify, authorize, or publish the release.";

/// Terminal state of one preparation reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePreparationStateV1 {
    Complete,
    Incomplete,
    DecisionRequired,
    Stale,
    Mismatch,
    Conflict,
    Unsupported,
    InstrumentFailure,
}

/// Result class of one orchestrated post-apply validation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateValidationResultV1 {
    /// Executed in-process and passed.
    Passed,
    /// Executed in-process and failed.
    Failed,
    /// Not executable here: the exact command is retained as an obligation
    /// for the operator or the next qualification child. Never fabricated.
    Deferred,
}

/// One orchestrated post-apply validation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateValidationRowV1 {
    pub obligation_id: String,
    pub command: String,
    pub result: CandidateValidationResultV1,
    pub detail: String,
}

/// One changed file with its before/after digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateChangedFileV1 {
    pub path: String,
    pub before_digest: Option<String>,
    pub after_digest: String,
}

/// One resolved repository decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateResolvedDecisionV1 {
    pub decision_id: String,
    pub resolution: String,
}

/// One selected release-graph row as reconciled post-apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateGraphRowV1 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub product_family: String,
    pub version: String,
}

/// The final typed preparation receipt (#3834).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePreparationReceiptV1 {
    pub schema: String,
    /// Digest of the applied plan.
    pub plan_digest: String,
    /// State of the intermediate apply receipt this reconciliation
    /// consumed.
    pub apply_state: String,
    /// Repository/worktree identity before the apply.
    pub before_identity_digest: String,
    /// Repository/worktree identity observed after the apply.
    pub after_identity_digest: String,
    /// Target release identity the candidate was prepared to.
    pub release_version: String,
    pub release_tag: String,
    pub release_channel: String,
    /// The reconciled release-graph rows (logical/package/version).
    pub selected_graph: Vec<CandidateGraphRowV1>,
    pub changed_files: Vec<CandidateChangedFileV1>,
    pub resolved_decisions: Vec<CandidateResolvedDecisionV1>,
    pub outstanding_decisions: Vec<String>,
    /// Result class of the Changie/history reconciliation row.
    pub changie_result: String,
    /// Result class of the release/support/channel coherence row.
    pub release_support_projection: String,
    /// Result class of the governed-file policy drift row.
    pub policy_drift_result: String,
    /// Result class of the no-op rerun row.
    pub no_op_rerun_result: String,
    pub validation_rows: Vec<CandidateValidationRowV1>,
    pub remaining_obligations: Vec<String>,
    pub state: CandidatePreparationStateV1,
    pub reasons: Vec<String>,
    pub claim_boundary: String,
}

impl CandidatePreparationReceiptV1 {
    /// Start a receipt with the fixed framing fields.
    pub fn new(
        plan_digest: String,
        apply_state: String,
        before_identity_digest: String,
        after_identity_digest: String,
        release_version: String,
        release_tag: String,
        release_channel: String,
    ) -> Self {
        Self {
            schema: CANDIDATE_PREPARATION_RECEIPT_SCHEMA_V1.to_string(),
            plan_digest,
            apply_state,
            before_identity_digest,
            after_identity_digest,
            release_version,
            release_tag,
            release_channel,
            selected_graph: Vec::new(),
            changed_files: Vec::new(),
            resolved_decisions: Vec::new(),
            outstanding_decisions: Vec::new(),
            changie_result: "deferred".to_string(),
            release_support_projection: "not_checked".to_string(),
            policy_drift_result: "not_checked".to_string(),
            no_op_rerun_result: "not_checked".to_string(),
            validation_rows: Vec::new(),
            remaining_obligations: Vec::new(),
            state: CandidatePreparationStateV1::InstrumentFailure,
            reasons: Vec::new(),
            claim_boundary: CANDIDATE_PREPARATION_RECEIPT_CLAIM_BOUNDARY_V1.to_string(),
        }
    }
}
