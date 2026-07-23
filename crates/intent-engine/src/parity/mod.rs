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
