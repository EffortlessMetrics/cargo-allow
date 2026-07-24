//! Phase-gate evaluation for proof-engine orchestration (#2589-A).

use proof_protocol::{ProofPhaseGatePostureV1, ProofPhaseGateV1, validate_phase_gate};

use crate::captured_receipts::{CapturedReceiptStoreV1, validate_captured_receipt_store};

pub const PHASE_GATE_EVALUATION_SCHEMA_ID: &str = "proof.phase-gate-evaluation.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseGateOutcomeV1 {
    Open,
    Blocked,
    Advisory,
}

impl PhaseGateOutcomeV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Blocked => "blocked",
            Self::Advisory => "advisory",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseGateEvaluationV1 {
    pub schema_id: String,
    pub phase_id: String,
    pub plan_id: String,
    pub outcome: PhaseGateOutcomeV1,
    pub missing_binding_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseGateError {
    CapturedReceipt(crate::captured_receipts::CapturedReceiptError),
    Protocol(proof_protocol::ProofPhaseGateError),
}

impl PhaseGateError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapturedReceipt(_) => "captured_receipt_invalid",
            Self::Protocol(_) => "phase_gate_invalid",
        }
    }
}

pub fn evaluate_phase_gate(
    gate: &ProofPhaseGateV1,
    store: &CapturedReceiptStoreV1,
) -> Result<PhaseGateEvaluationV1, PhaseGateError> {
    validate_phase_gate(gate).map_err(PhaseGateError::Protocol)?;
    validate_captured_receipt_store(store).map_err(PhaseGateError::CapturedReceipt)?;

    let captured_binding_ids: std::collections::BTreeSet<String> = store
        .get(&gate.plan_id)
        .map(|set| {
            set.bindings
                .iter()
                .map(|binding| binding.binding_id.clone())
                .collect()
        })
        .unwrap_or_default();

    let missing_binding_ids: Vec<String> = gate
        .required_binding_ids
        .iter()
        .filter(|binding_id| !captured_binding_ids.contains(*binding_id))
        .cloned()
        .collect();

    let outcome = if missing_binding_ids.is_empty() {
        PhaseGateOutcomeV1::Open
    } else {
        match gate.posture {
            ProofPhaseGatePostureV1::Blocking => PhaseGateOutcomeV1::Blocked,
            ProofPhaseGatePostureV1::Advisory => PhaseGateOutcomeV1::Advisory,
        }
    };

    Ok(PhaseGateEvaluationV1 {
        schema_id: PHASE_GATE_EVALUATION_SCHEMA_ID.to_string(),
        phase_id: gate.phase_id.clone(),
        plan_id: gate.plan_id.clone(),
        outcome,
        missing_binding_ids,
    })
}
