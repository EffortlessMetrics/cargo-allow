//! Proof plan transport and provider-neutral contracts for three-product
//! extraction (#2588).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-protocol` defines proof plan DTOs and provider-neutral transport. It
//! does not scan source files, does not invoke Cargo, compile code, execute
//! repository code, spawn processes, or depend on intent crates.

#[cfg(test)]
mod boundary;
mod capability_dtos;
mod contradiction_dtos;
mod parity;
mod phase_gate_dtos;
mod plan_dtos;
mod proof_corpus;
mod receipt_dtos;

#[cfg(test)]
mod tests;

pub use capability_dtos::{
    PROOF_CAPABILITY_CATALOG_SCHEMA_ID, ProofCapabilityCatalogV1, ProofCapabilityError,
    ProofCapabilityKindV1, ProofCapabilityV1, validate_capability_catalog,
};
pub use contradiction_dtos::{
    PROOF_CONTRADICTION_REPORT_SCHEMA_ID, ProofContradictionError, ProofContradictionReportV1,
    ProofContradictionV1, validate_contradiction_report,
};
pub use parity::{
    CapabilityDtosParityContract, ContradictionDtosParityContract, PhaseGateDtosParityContract,
    PlanDtosParityContract, ProofCorpusParityContract, ReceiptDtosParityContract,
    capability_dtos_parity_contract_path, capability_dtos_parity_contract_paths,
    contradiction_dtos_parity_contract_path, contradiction_dtos_parity_contract_paths,
    load_capability_dtos_parity_contract, load_contradiction_dtos_parity_contract,
    load_phase_gate_dtos_parity_contract, load_plan_dtos_parity_contract,
    load_proof_corpus_contract, load_proof_corpus_fixture, load_receipt_dtos_parity_contract,
    parity_contract_path, parity_contract_paths, phase_gate_dtos_parity_contract_path,
    phase_gate_dtos_parity_contract_paths, plan_dtos_parity_contract_path,
    plan_dtos_parity_contract_paths, proof_corpus_contract_path, proof_corpus_contract_paths,
    proof_corpus_fixture_path, receipt_dtos_parity_contract_path,
    receipt_dtos_parity_contract_paths,
};
pub use phase_gate_dtos::{
    PROOF_PHASE_GATE_SCHEMA_ID, ProofPhaseGateError, ProofPhaseGatePostureV1, ProofPhaseGateV1,
    validate_phase_gate,
};
pub use plan_dtos::{
    PROOF_PLAN_COMMAND_SCHEMA_ID, PROOF_PLAN_SCHEMA_ID, ProofPlanCommandV1, ProofPlanError,
    ProofPlanV1, load_proof_plan_toml, validate_proof_plan,
};
pub use proof_corpus::{
    BindingCurrentnessV1, PROOF_CORPUS_DIGEST_V1, PROOF_CORPUS_SCHEMA_ID, ProofBindingIdentityV1,
    ProofCorpusDimensionV1, ProofCorpusScenarioV1, ProofCorpusV1, ProofResultStateV1,
    ProviderEnvelopeV1, RIPR_EXTERNAL_PROOF_PROFILE_ID, load_proof_corpus_toml,
};
pub use receipt_dtos::{
    PROOF_RECEIPT_BINDING_SCHEMA_ID, PROOF_RECEIPT_SET_SCHEMA_ID, ProofReceiptBindingV1,
    ProofReceiptError, ProofReceiptSetV1, validate_receipt_set,
};
