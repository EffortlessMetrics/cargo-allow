use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RiprRoutingParityContract {
    pub scenario_id: String,
    pub proof_engine_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub profile_id: String,
    pub corpus_digest: String,
    pub required_phases: Vec<String>,
}

pub fn ripr_routing_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-engine/ripr-routing-v1.toml")
}

pub fn ripr_routing_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-engine/ripr-routing-contract-v1.toml")
}

pub fn load_ripr_routing_contract(path: &Path) -> Result<RiprRoutingParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
