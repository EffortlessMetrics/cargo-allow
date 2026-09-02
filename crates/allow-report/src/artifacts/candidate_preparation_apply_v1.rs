//! Intermediate apply receipt for one candidate-preparation transaction
//! (#3833).
//!
//! The apply engine writes only the deterministic generated set of one
//! exact reviewed plan through the repository's shared write-safety
//! authorities, all-or-nothing, with rollback from in-memory preimages.
//! This receipt is deliberately bounded: it records mechanics, not
//! release meaning — post-apply semantic reconciliation and the governed
//! handoff belong to the next child (#3834).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CANDIDATE_APPLY_RECEIPT_SCHEMA_V1: &str = "cargo-allow.candidate-apply-receipt.v1";

pub const CANDIDATE_APPLY_CLAIM_BOUNDARY_V1: &str = "Atomic, stale-safe application of one exact reviewed candidate-preparation plan using the repository's shared write-safety authorities. It changes only the declared source state; it does not determine release meaning, qualify bytes, authorize, or publish.";

/// Terminal classification of one apply attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateApplyStateV1 {
    Applied,
    NoOp,
    Stale,
    Mismatch,
    DecisionRequired,
    Conflict,
    RolledBack,
    RecoveryRequired,
    InstrumentFailure,
}

/// Per-operation record: what was intended and what was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateApplyOperationRecordV1 {
    pub owner: String,
    pub role: String,
    pub path: String,
    /// Posture as compiled by the reviewed plan.
    pub intended_posture: String,
    pub before_digest: Option<String>,
    pub staged_digest: Option<String>,
    pub after_digest: Option<String>,
    /// applied | noop | not_applied | rolled_back | recovery_required
    pub result: String,
}

/// Lock acquisition record for one underlying write target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateApplyLockRecordV1 {
    pub path: String,
    pub fingerprint: String,
    pub acquired: bool,
}

/// The bounded intermediate apply receipt (#3833).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateApplyReceiptV1 {
    pub schema: String,
    /// Digest of the reviewed plan that was applied.
    pub plan_digest: String,
    /// Repository/worktree identity observed before any write.
    pub before_identity_digest: String,
    pub locks: Vec<CandidateApplyLockRecordV1>,
    pub operations: Vec<CandidateApplyOperationRecordV1>,
    /// `true` when every staged byte matched its expected digest before
    /// the first commit.
    pub staged_validation: bool,
    /// Machine result class of the transaction itself.
    pub transaction_result: String,
    /// Machine result class of rollback/recovery, when rollback ran.
    pub rollback_result: String,
    /// Decisions acknowledged by the operator, keyed by decision id.
    pub decision_acknowledgements: BTreeMap<String, String>,
    /// Validation obligations that remain open after this apply.
    pub remaining_obligations: Vec<String>,
    pub state: CandidateApplyStateV1,
    pub reasons: Vec<String>,
    pub claim_boundary: String,
}

impl CandidateApplyReceiptV1 {
    /// Start a receipt with the fixed framing fields.
    pub fn new(plan_digest: String, before_identity_digest: String) -> Self {
        Self {
            schema: CANDIDATE_APPLY_RECEIPT_SCHEMA_V1.to_string(),
            plan_digest,
            before_identity_digest,
            locks: Vec::new(),
            operations: Vec::new(),
            staged_validation: false,
            transaction_result: "not_attempted".to_string(),
            rollback_result: "not_attempted".to_string(),
            decision_acknowledgements: BTreeMap::new(),
            remaining_obligations: Vec::new(),
            state: CandidateApplyStateV1::InstrumentFailure,
            reasons: Vec::new(),
            claim_boundary: CANDIDATE_APPLY_CLAIM_BOUNDARY_V1.to_string(),
        }
    }
}
