//! Change status report projection (#2599-B; DTO authority moved to
//! intent-protocol in #3309). Rendering stays here: the frame trait is
//! cargo-intent-owned, the stable operation DTO is protocol-owned.

use crate::render::RenderFrame;
pub use intent_protocol::{
    CHANGE_STATUS_CLAIM_BOUNDARY, CHANGE_STATUS_SCHEMA_ID, ChangeStatusReportV1, StagedChangeV1,
};

impl RenderFrame for ChangeStatusReportV1 {
    fn summary_line(&self) -> String {
        format!(
            "change status phase={} result={} obligations={}",
            self.phase, self.result_class, self.obligation_plan.open_obligation_count
        )
    }
}
