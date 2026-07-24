//! Snapshot-bound read-only cargo-allow proof provider (#2567 / #2554).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-adapter-cargo-allow` discovers an installed `cargo-allow` binary via public
//! process protocol, advertises reviewed command capabilities, and compiles dry-run
//! invocation specs without importing `cargo-allow` private crates.

mod boundary;
mod cargo_allow_provider;
mod cargo_allow_provider_surface;
mod digest;
mod parity;
mod process_protocol;
mod process_protocol_surface;
mod provider_contract;
mod provider_contract_surface;
mod provider_discovery;
mod provider_discovery_surface;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use cargo_allow_provider::CargoAllowProofProviderV1;
pub use cargo_allow_provider_surface::CargoAllowProviderSurface;
pub use process_protocol::{
    ProcessProtocolError, compile_cargo_allow_dry_run, validate_process_protocol_plan,
};
pub use process_protocol_surface::ProcessProtocolSurface;
pub use provider_contract::{
    CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID, CARGO_ALLOW_PROOF_PROVIDER_ID,
    CargoAllowProviderContractV1, ProviderAccessPostureV1, ProviderContractError,
    default_cargo_allow_provider_contract, validate_provider_contract,
};
pub use provider_contract_surface::ProviderContractSurface;
pub use provider_discovery::{
    CargoAllowDiscoveryMode, CargoAllowProviderFailure, CargoAllowProviderFailureClass,
    CargoAllowProviderRequest, CargoAllowProviderResolution, discover_cargo_allow_provider,
};
pub use provider_discovery_surface::ProviderDiscoverySurface;
pub use parity::{
    ProviderContractParityContract, load_provider_contract_parity_contract,
    parity_contract_path, parity_contract_paths, provider_contract_parity_contract_path,
    provider_contract_parity_contract_paths,
};
