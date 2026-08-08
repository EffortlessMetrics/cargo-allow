//! Cargo-allow proof provider (#2554, absorbed into cargo-proof #2938).
mod adapter;
mod contract;
mod digest;
mod discovery;
mod process_protocol;

pub use adapter::CargoAllowProofProviderV1;
pub use contract::{
    CARGO_ALLOW_PROOF_PROVIDER_ID, CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID,
    CargoAllowProviderContractV1, ProviderAccessPostureV1, ProviderContractError,
    default_cargo_allow_provider_contract, validate_provider_contract,
};
pub use discovery::{
    CargoAllowDiscoveryMode, CargoAllowProviderFailure, CargoAllowProviderFailureClass,
    CargoAllowProviderRequest, CargoAllowProviderResolution, discover_cargo_allow_provider,
};
pub use process_protocol::{
    ProcessProtocolError, compile_cargo_allow_dry_run, validate_process_protocol_plan,
};
