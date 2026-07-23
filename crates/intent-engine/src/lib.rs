//! Intent evaluator packets for three-product extraction (#2586).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-engine` orchestrates spec-system evaluation from intent-model domain
//! facts and intent-protocol transport envelopes. It parses source-tree inputs
//! without executing repository code and does not invoke Cargo, rustc, Clippy,
//! build scripts, proc macros, or proof commands.

mod engine_surface;
mod packet;
mod parity;
mod workspace;
mod workspace_surface;

pub use engine_surface::EvaluatorPacketSurface;
pub use packet::{
    INTENT_ENGINE_PACKET_SCHEMA_ID, INTENT_QUERY_TRANSPORT_SCHEMA_ID, IntentEnginePacketEnvelopeV1,
    IntentEnginePacketKindV1,
};
pub use parity::{
    EvaluatorPacketParityContract, WorkspaceCompositionParityContract,
    evaluator_packet_parity_contract_path, evaluator_packet_parity_contract_paths,
    load_evaluator_packet_parity_contract, load_self_hosted_workspace_composition_fixture,
    load_workspace_composition_parity_contract, self_hosted_workspace_composition_fixture_path,
    workspace_composition_parity_contract_path, workspace_composition_parity_contract_paths,
};
pub use workspace::{
    AUTHORITY_COMPILE_PLAN_SCHEMA_ID, AuthorityCompilePlanV1, AuthoritySourceRoleV1,
    AuthoritySourceV1, SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID, WorkspaceCompositionV1,
    composition_sources_present, load_workspace_composition_toml, plan_authority_compile,
};
pub use workspace_surface::WorkspaceCompilerSurface;

#[cfg(test)]
mod tests;
