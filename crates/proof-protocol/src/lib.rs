//! Proof protocol data seam: DTOs, serialization, and structural validation
//! (#2588 / #2943 step 6).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-protocol` defines the stable data/serialization/structural-validation
//! seam shared by the proof family.
//!
//! ## Data versus semantic boundary
//!
//! This crate owns **data only**: schema IDs, DTO types, TOML/JSON
//! serialization, and structural validation (required fields, ID shape,
//! local uniqueness, schema generation, enum/shape consistency expressible
//! without external or current state).
//!
//! **Semantic evaluation lives in proof-engine** (the sole semantic
//! evaluator): currentness against captured receipts, cache decisions,
//! blocking aggregation, contradiction interpretation, phase-gate
//! evaluation, provider registry behavior, and obligation planning. A raw
//! process or provider success can never be interpreted as obligation
//! satisfaction inside this crate.
//!
//! `proof-protocol` does not scan source files, does not invoke Cargo,
//! compile code, execute repository code, spawn processes, and does not
//! depend on intent, engine, or application crates.

#[cfg(test)]
mod boundary;
mod capability_dtos;
mod contradiction_dtos;
#[cfg(test)]
mod parity;
mod phase_gate_dtos;
mod plan_dtos;
mod plan_v2;
mod proof_corpus;
mod receipt_dtos;
mod receipt_status;

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
pub use phase_gate_dtos::{
    PROOF_PHASE_GATE_SCHEMA_ID, ProofPhaseGateError, ProofPhaseGatePostureV1, ProofPhaseGateV1,
    validate_phase_gate,
};
pub use plan_dtos::{
    PROOF_PLAN_COMMAND_SCHEMA_ID, PROOF_PLAN_SCHEMA_ID, ProofPlanCommandV1, ProofPlanError,
    ProofPlanV1, load_proof_plan_toml, validate_proof_plan,
};
pub use plan_v2::{
    ExpectedReceiptContractV1, PROOF_PLAN_V2_SCHEMA_ID, PROOF_PLAN_V2_SCHEMA_VERSION,
    ProofItemDispositionV1, ProofItemExecutionPostureV1, ProofItemV1, ProofPlanV2,
    ProofSubjectClassV1, ProofSubjectV1, ProviderSelectionV1,
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
pub use receipt_status::{
    CapturedReceiptManifestRowV1, CapturedReceiptManifestV1, PROOF_RECEIPT_MANIFEST_SCHEMA_ID,
    ProofItemReceiptStatusV1, validate_captured_receipt_manifest,
};
