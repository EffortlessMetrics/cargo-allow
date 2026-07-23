//! Intent-facing transport envelopes for three-product extraction (#2585).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-protocol` defines provider-neutral identity and query envelopes for
//! cargo-intent read paths. It does not scan repositories, execute proof commands,
//! or embed provider argv / RIPR / Hawk dialect surfaces.

mod identity;
mod parity;
mod protocol_surface;
mod query;

pub use identity::{INTENT_IDENTITY_SCHEMA_ID, IntentArtifactKindV1, IntentIdentityEnvelopeV1};
pub use parity::{
    IdentityQueryParityContract, identity_query_parity_contract_path,
    identity_query_parity_contract_paths, load_identity_query_parity_contract,
};
pub use protocol_surface::IdentityQuerySurface;
pub use query::{
    INTENT_QUERY_RESPONSE_SCHEMA_ID, INTENT_QUERY_SCHEMA_ID, IntentQueryEnvelopeV1,
    IntentQueryKindV1, IntentQueryResponseV1,
};

#[cfg(test)]
mod tests;
