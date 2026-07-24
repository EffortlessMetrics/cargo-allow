//! Contradiction detection for captured receipts (#2589-A).

use proof_protocol::{
    ProofContradictionReportV1, ProofContradictionV1, validate_contradiction_report,
};

use crate::captured_receipts::{CapturedReceiptStoreV1, validate_captured_receipt_store};
use crate::currentness::receipt_set_digest;

pub fn detect_contradictions(
    store: &CapturedReceiptStoreV1,
    plan_id: &str,
    expected_digest: &str,
) -> Result<ProofContradictionReportV1, ContradictionError> {
    validate_captured_receipt_store(store).map_err(ContradictionError::CapturedReceipt)?;
    let Some(set) = store.get(plan_id) else {
        return Ok(ProofContradictionReportV1::new(
            plan_id,
            vec![ProofContradictionV1 {
                contradiction_id: format!("{plan_id}.missing_receipts"),
                statement: "captured receipts missing for plan".to_string(),
                evidence_refs: Vec::new(),
            }],
        ));
    };
    let observed = receipt_set_digest(set);
    if observed == expected_digest {
        return Ok(ProofContradictionReportV1::new(plan_id, Vec::new()));
    }
    Ok(ProofContradictionReportV1::new(
        plan_id,
        vec![ProofContradictionV1 {
            contradiction_id: format!("{plan_id}.digest_mismatch"),
            statement: format!("expected digest {expected_digest}, observed {observed}"),
            evidence_refs: vec![plan_id.to_string()],
        }],
    ))
}

pub fn validate_engine_contradiction_report(
    report: &ProofContradictionReportV1,
) -> Result<(), ContradictionError> {
    validate_contradiction_report(report).map_err(ContradictionError::Protocol)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContradictionError {
    CapturedReceipt(crate::captured_receipts::CapturedReceiptError),
    Protocol(proof_protocol::ProofContradictionError),
}

impl ContradictionError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapturedReceipt(_) => "captured_receipt_invalid",
            Self::Protocol(_) => "contradiction_report_invalid",
        }
    }
}
