//! Intent-facing transport envelopes for three-product extraction (#2585).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-protocol` defines provider-neutral identity and query envelopes for
//! cargo-intent read paths. It parses source-tree transport contracts without
//! executing repository code and does not invoke Cargo, rustc, Clippy, build
//! scripts, proc macros, or proof commands.

mod identity;
mod parity;
mod protocol_surface;
mod query;
mod snapshot_package;

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
pub use snapshot_package::repo_protocol::{
    REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotKindV1, RepositorySnapshotV1,
    ResolvedRevisionV1, ResultClassV1, SelectedPathIdentityV1,
};

#[cfg(test)]
mod tests;
