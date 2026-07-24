//! Proof phase-gate transport (#2588-B+).

use serde::{Deserialize, Serialize};

pub const PROOF_PHASE_GATE_SCHEMA_ID: &str = "proof.phase-gate.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofPhaseGatePostureV1 {
    Blocking,
    Advisory,
}

impl ProofPhaseGatePostureV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Advisory => "advisory",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPhaseGateV1 {
    pub schema_id: String,
    pub phase_id: String,
    pub plan_id: String,
    pub required_binding_ids: Vec<String>,
    pub posture: ProofPhaseGatePostureV1,
}

impl ProofPhaseGateV1 {
    pub fn new(
        phase_id: impl Into<String>,
        plan_id: impl Into<String>,
        required_binding_ids: Vec<String>,
        posture: ProofPhaseGatePostureV1,
    ) -> Self {
        Self {
            schema_id: PROOF_PHASE_GATE_SCHEMA_ID.to_string(),
            phase_id: phase_id.into(),
            plan_id: plan_id.into(),
            required_binding_ids,
            posture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofPhaseGateError {
    InvalidSchemaId { observed: String },
    EmptyPhaseId,
    EmptyRequiredBindings,
}

impl ProofPhaseGateError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyPhaseId => "empty_phase_id",
            Self::EmptyRequiredBindings => "empty_required_bindings",
        }
    }
}

pub fn validate_phase_gate(gate: &ProofPhaseGateV1) -> Result<(), ProofPhaseGateError> {
    if gate.schema_id != PROOF_PHASE_GATE_SCHEMA_ID {
        return Err(ProofPhaseGateError::InvalidSchemaId {
            observed: gate.schema_id.clone(),
        });
    }
    if gate.phase_id.trim().is_empty() {
        return Err(ProofPhaseGateError::EmptyPhaseId);
    }
    if gate.required_binding_ids.is_empty() {
        return Err(ProofPhaseGateError::EmptyRequiredBindings);
    }
    Ok(())
}
