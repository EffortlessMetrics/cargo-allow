//! Intent evaluator packets for three-product extraction (#2586).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-engine` orchestrates spec-system evaluation from intent-model domain
//! facts and intent-protocol transport envelopes. It parses source-tree inputs
//! without executing repository code and does not invoke Cargo, rustc, Clippy,
//! build scripts, proc macros, or proof commands.

mod domain_queries;
mod graph_comparison;
mod graph_compiler;
mod packet;
mod parity;
mod parity_corpus;
mod phase_obligations;
mod subject_resolution;
mod workspace;

pub use domain_queries::{
    BOUNDED_DOMAIN_QUERY_RESPONSE_SCHEMA_ID, BOUNDED_DOMAIN_QUERY_SCHEMA_ID,
    BoundedDomainQueryKindV1, BoundedDomainQueryRequestV1, BoundedDomainQueryResponseV1,
    RESULT_CLASS_COMPLETED, canonical_bounded_domain_query_kinds, execute_bounded_domain_query,
    load_bounded_domain_query_catalog_toml, to_intent_query_response_json,
};
pub use graph_comparison::{
    GRAPH_COMPARISON_REPORT_SCHEMA_ID, GraphComparisonReportV1, GraphMovementKindV1,
    GraphMovementV1, canonical_graph_movement_kinds, load_graph_comparison_report_json,
    sort_graph_movements,
};
pub use graph_compiler::compile_spec_graph;
pub use packet::{
    INTENT_ENGINE_PACKET_SCHEMA_ID, IntentEnginePacketEnvelopeV1, IntentEnginePacketKindV1,
};
pub use parity::{
    BoundedDomainQueriesParityContract, EvaluatorPacketParityContract,
    GraphComparisonParityContract, ParityCorpusParityContract, PhaseObligationsParityContract,
    WorkspaceCompositionParityContract, bounded_domain_queries_parity_contract_path,
    bounded_domain_queries_parity_contract_paths, bounded_domain_query_catalog_fixture_path,
    evaluator_packet_parity_contract_path, evaluator_packet_parity_contract_paths,
    graph_comparison_parity_contract_path, graph_comparison_parity_contract_paths,
    graph_movement_kinds_fixture_path, load_bounded_domain_queries_parity_contract,
    load_bounded_domain_query_catalog_fixture, load_evaluator_packet_parity_contract,
    load_graph_comparison_parity_contract, load_graph_movement_kinds_fixture,
    load_parity_corpus_contract, load_parity_corpus_fixture,
    load_phase_obligations_parity_contract, load_precommit_obligation_plan_fixture,
    load_self_hosted_workspace_composition_fixture, load_workspace_composition_parity_contract,
    parity_corpus_contract_path, parity_corpus_contract_paths, parity_corpus_fixture_path,
    phase_obligations_parity_contract_path, phase_obligations_parity_contract_paths,
    precommit_obligation_plan_fixture_path, self_hosted_workspace_composition_fixture_path,
    workspace_composition_parity_contract_path, workspace_composition_parity_contract_paths,
};
pub use parity_corpus::{
    PARITY_CORPUS_DIGEST_V1, PARITY_CORPUS_SCHEMA_ID, ParityCorpusDimensionV1,
    ParityCorpusScenarioV1, ParityCorpusV1, canonical_parity_dispositions, load_parity_corpus_toml,
    validate_parity_corpus,
};
pub use phase_obligations::{
    InventoryPostureV1, ObligationPostureV1, PHASE_OBLIGATION_PLAN_SCHEMA_ID, PRECOMMIT_PHASE_ID,
    PhaseObligationCompileInputV1, PhaseObligationItemV1, PhaseObligationKindV1,
    PhaseObligationPlanV1, compile_phase_obligation_plan, load_phase_obligation_plan_toml,
};
pub use subject_resolution::{
    IntentSubjectResolutionClassV1, IntentSubjectResolutionV1, resolve_authored_rust_subject,
};
pub use workspace::{
    AUTHORITY_COMPILE_PLAN_SCHEMA_ID, AuthorityCompilePlanV1, AuthoritySourceRoleV1,
    AuthoritySourceV1, SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID, WorkspaceCompositionV1,
    composition_sources_present, load_workspace_composition_toml, plan_authority_compile,
};

#[cfg(test)]
mod tests;
