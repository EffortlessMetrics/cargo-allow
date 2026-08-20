//! Feature-gated proof provider modules (#2938).
//!
//! Each provider is feature-gated so cargo-proof can be built with only the
//! providers needed for a given deployment. The `all-providers` feature
//! enables all three.

#[cfg(feature = "provider-cargo-allow")]
pub mod cargo_allow;
#[cfg(feature = "provider-hawk")]
pub mod hawk;
#[cfg(feature = "provider-ripr")]
pub mod ripr;

mod registry;
pub use registry::{
    PROVIDER_REGISTRY_SCHEMA_ID, ProviderAvailabilityV1, ProviderDispositionV1,
    ProviderProjectionV1, ProviderRegistryError, StaticProviderRegistryV1,
};
