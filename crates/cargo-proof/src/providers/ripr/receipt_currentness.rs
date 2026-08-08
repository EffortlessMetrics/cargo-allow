//! RIPR receipt currentness evaluation (#2556).

use super::grip_receipt::{RiprGripReceiptV1, validate_ripr_grip_receipt};

pub const RIPR_RECEIPT_CURRENTNESS_SCHEMA_ID: &str = "proof.ripr-receipt-currentness.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiprReceiptCurrentnessStatusV1 {
    Current,
    StaleSnapshot,
    SubjectMismatch,
    SeamMismatch,
    RequirementMismatch,
    MalformedReceipt,
}

impl RiprReceiptCurrentnessStatusV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::StaleSnapshot => "stale_snapshot",
            Self::SubjectMismatch => "subject_mismatch",
            Self::SeamMismatch => "seam_mismatch",
            Self::RequirementMismatch => "requirement_mismatch",
            Self::MalformedReceipt => "malformed_receipt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiprReceiptCurrentnessReportV1 {
    pub schema_id: String,
    pub receipt_id: String,
    pub status: RiprReceiptCurrentnessStatusV1,
    pub observed_snapshot_digest: String,
    pub expected_snapshot_digest: String,
}

pub struct RiprCurrentnessRequest<'a> {
    pub receipt: &'a RiprGripReceiptV1,
    pub expected_snapshot_digest: &'a str,
    pub expected_subject_ref: &'a str,
    pub expected_seam_ref: &'a str,
    pub expected_requirement_id: &'a str,
}

pub fn evaluate_receipt_currentness(
    request: &RiprCurrentnessRequest<'_>,
) -> RiprReceiptCurrentnessReportV1 {
    let receipt = request.receipt;
    let status = if validate_ripr_grip_receipt(receipt).is_err() {
        RiprReceiptCurrentnessStatusV1::MalformedReceipt
    } else if receipt.snapshot_digest != request.expected_snapshot_digest {
        RiprReceiptCurrentnessStatusV1::StaleSnapshot
    } else if receipt.subject_ref != request.expected_subject_ref {
        RiprReceiptCurrentnessStatusV1::SubjectMismatch
    } else if receipt.seam_ref != request.expected_seam_ref {
        RiprReceiptCurrentnessStatusV1::SeamMismatch
    } else if receipt.requirement_id != request.expected_requirement_id {
        RiprReceiptCurrentnessStatusV1::RequirementMismatch
    } else {
        RiprReceiptCurrentnessStatusV1::Current
    };
    RiprReceiptCurrentnessReportV1 {
        schema_id: RIPR_RECEIPT_CURRENTNESS_SCHEMA_ID.to_string(),
        receipt_id: receipt.receipt_id.clone(),
        status,
        observed_snapshot_digest: receipt.snapshot_digest.clone(),
        expected_snapshot_digest: request.expected_snapshot_digest.to_string(),
    }
}
