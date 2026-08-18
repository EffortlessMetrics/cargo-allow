mod corpus;
mod domain_queries;
mod graph_comparison;
mod graph_compiler;
mod phase_obligations;
mod workspace;

pub use corpus::{
    ParityCorpusParityContract, load_parity_corpus_contract, load_parity_corpus_fixture,
    parity_corpus_contract_path, parity_corpus_contract_paths, parity_corpus_fixture_path,
};
pub use domain_queries::{
    BoundedDomainQueriesParityContract, bounded_domain_queries_parity_contract_path,
    bounded_domain_queries_parity_contract_paths, bounded_domain_query_catalog_fixture_path,
    load_bounded_domain_queries_parity_contract, load_bounded_domain_query_catalog_fixture,
};
pub use graph_comparison::{
    GraphComparisonParityContract, graph_comparison_parity_contract_path,
    graph_comparison_parity_contract_paths, graph_movement_kinds_fixture_path,
    load_graph_comparison_parity_contract, load_graph_movement_kinds_fixture,
};
pub use graph_compiler::{
    GraphCompilerParityContract, GraphCompilerParityScenario, ParityDimensionRecord,
    graph_compiler_parity_contract_path, graph_compiler_parity_contract_paths,
    load_graph_compiler_parity_contract,
};
pub use phase_obligations::{
    PhaseObligationsParityContract, load_phase_obligations_parity_contract,
    load_precommit_obligation_plan_fixture, phase_obligations_parity_contract_path,
    phase_obligations_parity_contract_paths, precommit_obligation_plan_fixture_path,
};
pub use workspace::{
    WorkspaceCompositionParityContract, load_self_hosted_workspace_composition_fixture,
    load_workspace_composition_parity_contract, self_hosted_workspace_composition_fixture_path,
    workspace_composition_parity_contract_path, workspace_composition_parity_contract_paths,
};

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EvaluatorPacketParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    pub required_packet_fields: Vec<String>,
}

pub fn evaluator_packet_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-evaluator-packet-v1.toml")
}

pub fn evaluator_packet_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![evaluator_packet_parity_contract_path(root)]
}

pub fn load_evaluator_packet_parity_contract(
    path: &Path,
) -> Result<EvaluatorPacketParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
