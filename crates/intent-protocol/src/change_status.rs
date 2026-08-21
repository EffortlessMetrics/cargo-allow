//! Change-status operation DTOs (#3309, slice 7 of #2935).
//!
//! Stable operation DTOs live in `intent-protocol` per the converged
//! topology: cargo-intent produces these envelopes, and the one-way
//! cargo-allow compatibility path consumes them by schema contract. The
//! schema id is the single authority here; cargo-allow carries a mirror
//! literal (the dependency law forbids its production import) bound to
//! this constant by a dev-scope parity test.

use serde::Serialize;

use crate::{IntentObligationPlanResponseV1, IntentViewResponseV1};

pub const CHANGE_STATUS_SCHEMA_ID: &str = "cargo-intent.change-status.v1";
pub const CHANGE_STATUS_CLAIM_BOUNDARY: &str = "Exact staged source posture and phase obligation skeleton; no graph compilation, policy findings, project execution, or proof.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StagedChangeV1 {
    pub status: String,
    pub path: Option<String>,
    pub previous_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeStatusReportV1 {
    pub schema_id: String,
    pub command: String,
    pub phase: String,
    pub profile: String,
    pub staged_identity: String,
    pub staged_changes: Vec<StagedChangeV1>,
    pub inventory_completeness: String,
    pub staged_view: IntentViewResponseV1,
    pub obligation_plan: IntentObligationPlanResponseV1,
    pub unmapped_staged_surface: bool,
    pub result_class: String,
    pub process_exit_family: String,
    pub claim_boundary: String,
}
