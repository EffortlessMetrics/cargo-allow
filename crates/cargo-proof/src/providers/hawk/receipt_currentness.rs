//! Hawk receipt currentness evaluation (#2555).

use super::analysis_receipt::{HawkAnalysisReceiptV1, validate_hawk_analysis_receipt};

pub const HAWK_RECEIPT_CURRENTNESS_SCHEMA_ID: &str = "proof.hawk-receipt-currentness.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HawkReceiptCurrentnessStatusV1 {
    Current,
    StaleSnapshot,
    StaleConfig,
    StaleToolchain,
    StaleTarget,
    MalformedReceipt,
}

impl HawkReceiptCurrentnessStatusV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::StaleSnapshot => "stale_snapshot",
            Self::StaleConfig => "stale_config",
            Self::StaleToolchain => "stale_toolchain",
            Self::StaleTarget => "stale_target",
            Self::MalformedReceipt => "malformed_receipt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HawkReceiptCurrentnessReportV1 {
    pub schema_id: String,
    pub receipt_id: String,
    pub status: HawkReceiptCurrentnessStatusV1,
}

pub struct HawkCurrentnessRequest<'a> {
    pub receipt: &'a HawkAnalysisReceiptV1,
    pub expected_snapshot_digest: &'a str,
    pub expected_config_digest: &'a str,
    pub expected_rustc_release: &'a str,
    pub expected_target_triple: &'a str,
}

pub fn evaluate_hawk_receipt_currentness(
    request: &HawkCurrentnessRequest<'_>,
) -> HawkReceiptCurrentnessReportV1 {
    let receipt = request.receipt;
    let status = if validate_hawk_analysis_receipt(receipt).is_err() {
        HawkReceiptCurrentnessStatusV1::MalformedReceipt
    } else if receipt.snapshot_digest != request.expected_snapshot_digest {
        HawkReceiptCurrentnessStatusV1::StaleSnapshot
    } else if receipt.config_digest != request.expected_config_digest {
        HawkReceiptCurrentnessStatusV1::StaleConfig
    } else if receipt.rustc_release != request.expected_rustc_release {
        HawkReceiptCurrentnessStatusV1::StaleToolchain
    } else if receipt.target_triple != request.expected_target_triple {
        HawkReceiptCurrentnessStatusV1::StaleTarget
    } else {
        HawkReceiptCurrentnessStatusV1::Current
    };
    HawkReceiptCurrentnessReportV1 {
        schema_id: HAWK_RECEIPT_CURRENTNESS_SCHEMA_ID.to_string(),
        receipt_id: receipt.receipt_id.clone(),
        status,
    }
}
