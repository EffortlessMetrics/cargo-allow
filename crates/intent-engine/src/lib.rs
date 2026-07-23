//! Intent evaluator packets for three-product extraction (#2586).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-engine` orchestrates spec-system evaluation from intent-model domain
//! facts and intent-protocol transport envelopes. It parses source-tree inputs
//! without executing repository code and does not invoke Cargo, rustc, Clippy,
//! build scripts, proc macros, or proof commands.

mod engine_surface;
mod graph_comparison;
mod graph_comparison_surface;
mod packet;
mod parity;
mod phase_obligations;
mod phase_obligations_surface;
mod workspace;
mod workspace_surface;

pub use engine_surface::EvaluatorPacketSurface;
pub use graph_comparison::{
    GRAPH_COMPARISON_REPORT_SCHEMA_ID, GraphComparisonReportV1, GraphMovementKindV1,
    GraphMovementV1, canonical_graph_movement_kinds, load_graph_comparison_report_json,
    sort_graph_movements,
};
pub use graph_comparison_surface::GraphComparisonSurface;
pub use packet::{
    INTENT_ENGINE_PACKET_SCHEMA_ID, INTENT_QUERY_TRANSPORT_SCHEMA_ID, IntentEnginePacketEnvelopeV1,
    IntentEnginePacketKindV1,
};
pub use parity::{
    EvaluatorPacketParityContract, GraphComparisonParityContract, PhaseObligationsParityContract,
    WorkspaceCompositionParityContract, evaluator_packet_parity_contract_path,
    evaluator_packet_parity_contract_paths, graph_comparison_parity_contract_path,
    graph_comparison_parity_contract_paths, graph_movement_kinds_fixture_path,
    load_evaluator_packet_parity_contract, load_graph_comparison_parity_contract,
    load_graph_movement_kinds_fixture, load_phase_obligations_parity_contract,
    load_precommit_obligation_plan_fixture, load_self_hosted_workspace_composition_fixture,
    load_workspace_composition_parity_contract, phase_obligations_parity_contract_path,
    phase_obligations_parity_contract_paths, precommit_obligation_plan_fixture_path,
    self_hosted_workspace_composition_fixture_path, workspace_composition_parity_contract_path,
    workspace_composition_parity_contract_paths,
};
pub use phase_obligations::{
    InventoryPostureV1, ObligationPostureV1, PHASE_OBLIGATION_PLAN_SCHEMA_ID, PRECOMMIT_PHASE_ID,
    PhaseObligationCompileInputV1, PhaseObligationItemV1, PhaseObligationKindV1,
    PhaseObligationPlanV1, compile_phase_obligation_plan, load_phase_obligation_plan_toml,
};
pub use phase_obligations_surface::PhaseObligationsSurface;
pub use workspace::{
    AUTHORITY_COMPILE_PLAN_SCHEMA_ID, AuthorityCompilePlanV1, AuthoritySourceRoleV1,
    AuthoritySourceV1, SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID, WorkspaceCompositionV1,
    composition_sources_present, load_workspace_composition_toml, plan_authority_compile,
};
pub use workspace_surface::WorkspaceCompilerSurface;

#[cfg(test)]
mod tests;
