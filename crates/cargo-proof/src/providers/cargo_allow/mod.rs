//! Cargo-allow proof provider (#2554, absorbed into cargo-proof #2938).
#[cfg_attr(not(feature = "all-providers"), expect(dead_code))]
mod adapter;
mod digest;

pub use adapter::CargoAllowProofProviderV1;
// The facade re-exports exist for the public provider surface; cfg(test)
// builds of the crate legitimately consume them only through the glob in
// the submodule tests, so the unused-import allowance is scoped here with
// that exact reason (cargo-allow receipts this attribute).
#[cfg_attr(not(feature = "all-providers"), expect(dead_code))]
mod contract;
#[cfg_attr(not(feature = "all-providers"), expect(dead_code))]
mod discovery;
#[cfg_attr(not(feature = "all-providers"), expect(dead_code))]
mod process_protocol;
#[allow(unused_imports)]
pub use contract::{
    CARGO_ALLOW_PROOF_PROVIDER_ID, CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID,
    CargoAllowProviderContractV1, ProviderAccessPostureV1, ProviderContractError,
    default_cargo_allow_provider_contract, validate_provider_contract,
};
#[allow(unused_imports)]
pub use discovery::{
    CargoAllowDiscoveryMode, CargoAllowProviderFailure, CargoAllowProviderFailureClass,
    CargoAllowProviderRequest, CargoAllowProviderResolution, discover_cargo_allow_provider,
};
#[allow(unused_imports)]
pub use process_protocol::{
    ProcessProtocolError, compile_cargo_allow_dry_run, validate_process_protocol_plan,
};
