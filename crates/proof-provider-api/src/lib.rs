//! Proof provider API contracts for three-product extraction (#2603).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-provider-api` defines provider-neutral execution contracts over
//! `proof-protocol` transport. It does not scan source files, does not invoke
//! Cargo, compile code, execute repository code, spawn processes, or depend on
//! intent crates.

mod boundary;
mod conformance;
mod fake_provider;
mod parity;
mod provider_api;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use conformance::{
    CONFORMANCE_SCENARIO_ID, run_fake_provider_conformance, run_provider_conformance,
};
pub use fake_provider::{FAKE_PROOF_PROVIDER_ID, FakeProofProviderV1};
pub use parity::{parity_contract_path, parity_contract_paths};
pub use provider_api::{
    PROOF_PROVIDER_API_SCHEMA_ID, ProofProviderV1, ProviderApiError, validate_provider_plan,
    validate_provider_surface,
};
