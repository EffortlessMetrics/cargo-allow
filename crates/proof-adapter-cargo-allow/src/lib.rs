//! Snapshot-bound read-only cargo-allow proof provider (#2567 / #2554).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-adapter-cargo-allow` discovers an installed `cargo-allow` binary via public
//! process protocol, advertises reviewed command capabilities, and compiles dry-run
//! invocation specs without importing `cargo-allow` private crates. It does not scan
//! source files, does not invoke Cargo, compile code, execute repository code, or
//! depend on intent crates.

#[cfg(test)]
mod boundary;
mod cargo_allow_provider;
mod digest;
mod parity;
mod process_protocol;
mod provider_contract;
mod provider_discovery;

#[cfg(test)]
mod tests;

pub use cargo_allow_provider::CargoAllowProofProviderV1;
pub use parity::{
    ProviderContractParityContract, load_provider_contract_parity_contract, parity_contract_path,
    parity_contract_paths, provider_contract_parity_contract_path,
    provider_contract_parity_contract_paths,
};
pub use process_protocol::{
    ProcessProtocolError, compile_cargo_allow_dry_run, validate_process_protocol_plan,
};
pub use provider_contract::{
    CARGO_ALLOW_PROOF_PROVIDER_ID, CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID,
    CargoAllowProviderContractV1, ProviderAccessPostureV1, ProviderContractError,
    default_cargo_allow_provider_contract, validate_provider_contract,
};
pub use provider_discovery::{
    CargoAllowDiscoveryMode, CargoAllowProviderFailure, CargoAllowProviderFailureClass,
    CargoAllowProviderRequest, CargoAllowProviderResolution, discover_cargo_allow_provider,
};
