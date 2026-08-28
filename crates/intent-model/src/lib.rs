//! Intent domain types for three-product spec-system extraction (#2584).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-model` is an internal cargo-intent crate for spec-system domain facts.
//!
//! This crate owns authored spec-system configuration and domain DTOs. It
//! parses source-tree artifact bytes without executing repository code and does
//! not invoke Cargo, rustc, Clippy, build scripts, proc macros, or proof
//! commands.

extern crate self as allow_core;

mod agentic_candidate;
mod error;
mod governance_v2;
mod parity;
mod spec_system;

pub use agentic_candidate::{
    CLAIM_REF_SCHEMA_V1, CandidateAdmissionDecisionV1, CandidateDispositionV1,
    CandidateObservationSetV1, CandidateObservationV1, CandidateStateV1, ClaimRefV1,
};
pub use error::{
    IntentModelError, IntentModelErrorKind, IntentModelErrorLocation, IntentModelResult,
    normalize_path, read_text_file_capped, stable_hash_hex,
};
pub use error::{
    IntentModelError as CargoAllowError, IntentModelErrorKind as CargoAllowErrorKind,
    IntentModelResult as CargoAllowResult,
};
pub use governance_v2::*;
pub use parity::{
    SpecSystemParityContract, load_spec_system_parity_contract, spec_system_parity_contract_path,
    spec_system_parity_contract_paths,
};
pub use spec_system::*;

#[cfg(test)]
mod tests;
