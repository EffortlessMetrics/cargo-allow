//! Change status report projection (#2599-B).

use crate::render::RenderFrame;
use intent_protocol::{IntentObligationPlanResponseV1, IntentViewResponseV1};
use serde::Serialize;

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

impl RenderFrame for ChangeStatusReportV1 {
    fn summary_line(&self) -> String {
        format!(
            "change status phase={} result={} obligations={}",
            self.phase, self.result_class, self.obligation_plan.open_obligation_count
        )
    }
}
