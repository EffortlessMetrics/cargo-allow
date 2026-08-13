//! Intent-facing transport envelopes for three-product extraction (#2585).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-protocol` defines provider-neutral identity and query envelopes for
//! cargo-intent read paths. It parses source-tree transport contracts without
//! executing repository code and does not invoke Cargo, rustc, Clippy, build
//! scripts, proc macros, or proof commands.

mod closure;
mod diff;
mod identity;
mod obligation;
mod packet;
mod parity;
mod query;
mod snapshot_package;
mod view;

pub use closure::{
    INTENT_SOURCE_CLOSURE_RESPONSE_SCHEMA_ID, INTENT_SOURCE_CLOSURE_SCHEMA_ID,
    IntentSourceClosureEnvelopeV1, IntentSourceClosureResponseV1,
};
pub use diff::{
    INTENT_DIFF_RESPONSE_SCHEMA_ID, INTENT_DIFF_SCHEMA_ID, IntentDiffEnvelopeV1, IntentDiffKindV1,
    IntentDiffResponseV1,
};
pub use identity::{INTENT_IDENTITY_SCHEMA_ID, IntentArtifactKindV1, IntentIdentityEnvelopeV1};
pub use obligation::{
    INTENT_OBLIGATION_PLAN_RESPONSE_SCHEMA_ID, INTENT_OBLIGATION_PLAN_SCHEMA_ID,
    IntentObligationPlanEnvelopeV1, IntentObligationPlanResponseV1, IntentObligationPostureV1,
    IntentPhaseObligationKindV1, IntentPhaseObligationV1,
};
pub use packet::{
    INTENT_ENGINE_PACKET_SCHEMA_ID, IntentEnginePacketEnvelopeV1, IntentEnginePacketKindV1,
};
pub use parity::{
    IdentityQueryParityContract, ObligationPlanParityContract, ViewDiffClosureParityContract,
    identity_query_parity_contract_path, identity_query_parity_contract_paths,
    load_identity_query_parity_contract, load_obligation_plan_parity_contract,
    load_view_diff_closure_parity_contract, obligation_plan_parity_contract_path,
    obligation_plan_parity_contract_paths, view_diff_closure_parity_contract_path,
    view_diff_closure_parity_contract_paths,
};
pub use query::{
    INTENT_QUERY_RESPONSE_SCHEMA_ID, INTENT_QUERY_SCHEMA_ID, IntentQueryEnvelopeV1,
    IntentQueryKindV1, IntentQueryResponseV1,
};
pub use snapshot_package::repo_protocol::{
    REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotKindV1, RepositorySnapshotV1,
    ResolvedRevisionV1, ResultClassV1, SelectedPathIdentityV1,
};
pub use view::{
    INTENT_VIEW_RESPONSE_SCHEMA_ID, INTENT_VIEW_SCHEMA_ID, IntentViewEnvelopeV1, IntentViewKindV1,
    IntentViewResponseV1,
};

#[cfg(test)]
mod tests;
