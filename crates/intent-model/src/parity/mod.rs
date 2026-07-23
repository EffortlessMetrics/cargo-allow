use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpecSystemParityContract {
    pub scenario_id: String,
    pub allow_policy_module: String,
    pub intent_model_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_config_fields: Vec<String>,
}

pub fn spec_system_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-model/parity-spec-system-v1.toml")
}

pub fn spec_system_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![spec_system_parity_contract_path(root)]
}

pub fn load_spec_system_parity_contract(path: &Path) -> Result<SpecSystemParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
