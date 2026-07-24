//! Proof plan transport and provider-neutral contracts for three-product
//! extraction (#2588).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-protocol` defines proof plan DTOs and provider-neutral transport. It
//! does not scan source files, does not invoke Cargo, compile code, execute
//! repository code, spawn processes, or depend on intent crates.

mod boundary;
mod capability_dtos;
mod capability_dtos_surface;
mod contradiction_dtos;
mod contradiction_dtos_surface;
mod parity;
mod phase_gate_dtos;
mod phase_gate_dtos_surface;
mod plan_dtos;
mod plan_dtos_surface;
mod receipt_dtos;
mod receipt_dtos_surface;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use capability_dtos::{
    PROOF_CAPABILITY_CATALOG_SCHEMA_ID, ProofCapabilityCatalogV1, ProofCapabilityError,
    ProofCapabilityKindV1, ProofCapabilityV1, validate_capability_catalog,
};
pub use capability_dtos_surface::CapabilityDtosSurface;
pub use contradiction_dtos::{
    PROOF_CONTRADICTION_REPORT_SCHEMA_ID, ProofContradictionError, ProofContradictionReportV1,
    ProofContradictionV1, validate_contradiction_report,
};
pub use contradiction_dtos_surface::ContradictionDtosSurface;
pub use parity::{
    CapabilityDtosParityContract, ContradictionDtosParityContract, PhaseGateDtosParityContract,
    PlanDtosParityContract, ReceiptDtosParityContract, capability_dtos_parity_contract_path,
    capability_dtos_parity_contract_paths, contradiction_dtos_parity_contract_path,
    contradiction_dtos_parity_contract_paths, load_capability_dtos_parity_contract,
    load_contradiction_dtos_parity_contract, load_phase_gate_dtos_parity_contract,
    load_plan_dtos_parity_contract, load_receipt_dtos_parity_contract, parity_contract_path,
    parity_contract_paths, phase_gate_dtos_parity_contract_path,
    phase_gate_dtos_parity_contract_paths, plan_dtos_parity_contract_path,
    plan_dtos_parity_contract_paths, receipt_dtos_parity_contract_path,
    receipt_dtos_parity_contract_paths,
};
pub use phase_gate_dtos::{
    PROOF_PHASE_GATE_SCHEMA_ID, ProofPhaseGateError, ProofPhaseGatePostureV1, ProofPhaseGateV1,
    validate_phase_gate,
};
pub use phase_gate_dtos_surface::PhaseGateDtosSurface;
pub use plan_dtos::{
    PROOF_PLAN_COMMAND_SCHEMA_ID, PROOF_PLAN_SCHEMA_ID, ProofPlanCommandV1, ProofPlanError,
    ProofPlanV1, load_proof_plan_toml, validate_proof_plan,
};
pub use plan_dtos_surface::PlanDtosSurface;
pub use receipt_dtos::{
    PROOF_RECEIPT_BINDING_SCHEMA_ID, PROOF_RECEIPT_SET_SCHEMA_ID, ProofReceiptBindingV1,
    ProofReceiptError, ProofReceiptSetV1, validate_receipt_set,
};
pub use receipt_dtos_surface::ReceiptDtosSurface;
