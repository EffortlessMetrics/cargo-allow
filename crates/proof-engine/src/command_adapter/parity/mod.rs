//! Parity fixture discovery for proof-adapter-command (#2603-B).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CommandRegistryParityContract {
    pub scenario_id: String,
    pub proof_adapter_command_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_registry_fields: Vec<String>,
    pub required_spec_fields: Vec<String>,
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-command/parity-command-registry-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("tests/fixtures/proof-adapter-command/parity-boundary-v1.toml"),
        command_registry_parity_contract_path(root),
    ]
}

pub fn command_registry_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-command/parity-command-registry-v1.toml")
}

pub fn command_registry_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![command_registry_parity_contract_path(root)]
}

pub fn load_command_registry_parity_contract(
    path: &Path,
) -> Result<CommandRegistryParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
