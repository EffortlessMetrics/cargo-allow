//! Currentness evaluation for captured receipts (#2589-A).

use proof_protocol::ProofReceiptSetV1;

use crate::captured_receipts::{CapturedReceiptStoreV1, validate_captured_receipt_store};

pub const CURRENTNESS_REPORT_SCHEMA_ID: &str = "proof.currentness-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentnessStatusV1 {
    Current,
    Stale,
    Missing,
}

impl CurrentnessStatusV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentnessReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub status: CurrentnessStatusV1,
    pub observed_digest: Option<String>,
    pub expected_digest: Option<String>,
}

pub fn evaluate_currentness(
    store: &CapturedReceiptStoreV1,
    plan_id: &str,
    expected_digest: Option<&str>,
) -> Result<CurrentnessReportV1, CurrentnessError> {
    validate_captured_receipt_store(store).map_err(CurrentnessError::CapturedReceipt)?;
    let Some(set) = store.get(plan_id) else {
        return Ok(CurrentnessReportV1 {
            schema_id: CURRENTNESS_REPORT_SCHEMA_ID.to_string(),
            plan_id: plan_id.to_string(),
            status: CurrentnessStatusV1::Missing,
            observed_digest: None,
            expected_digest: expected_digest.map(str::to_string),
        });
    };
    let observed_digest = receipt_set_digest(set);
    let status = match expected_digest {
        Some(expected) if expected == observed_digest => CurrentnessStatusV1::Current,
        Some(_) => CurrentnessStatusV1::Stale,
        None => CurrentnessStatusV1::Current,
    };
    Ok(CurrentnessReportV1 {
        schema_id: CURRENTNESS_REPORT_SCHEMA_ID.to_string(),
        plan_id: plan_id.to_string(),
        status,
        observed_digest: Some(observed_digest),
        expected_digest: expected_digest.map(str::to_string),
    })
}

pub fn receipt_set_digest(set: &ProofReceiptSetV1) -> String {
    let mut digests: Vec<&str> = set
        .bindings
        .iter()
        .map(|b| b.receipt_digest.as_str())
        .collect();
    digests.sort_unstable();
    digests.join("|")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentnessError {
    CapturedReceipt(crate::captured_receipts::CapturedReceiptError),
}

impl CurrentnessError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapturedReceipt(_) => "captured_receipt_invalid",
        }
    }
}
