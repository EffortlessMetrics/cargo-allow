//! Proof provider API contracts and conformance harness (#2603-A).
//!
//! Absorbed into proof-engine from the standalone `proof-provider-api` crate
//! (#2937). Provider-neutral execution contracts over proof-protocol transport.

#[cfg(test)]
mod boundary;
mod conformance;
mod contracts;
mod fake_provider;
mod parity;

#[cfg(test)]
mod tests;

pub use conformance::{
    CONFORMANCE_SCENARIO_ID, run_fake_provider_conformance, run_provider_conformance,
};
pub use contracts::{
    PROOF_PROVIDER_API_SCHEMA_ID, ProofProviderV1, ProviderApiError, validate_provider_plan,
    validate_provider_surface,
};
pub use fake_provider::{FAKE_PROOF_PROVIDER_ID, FakeProofProviderV1};
pub use parity::{parity_contract_path, parity_contract_paths};
