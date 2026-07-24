//! Parity fixture discovery for proof-adapter-cargo-allow (#2567 / #2554).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProviderContractParityContract {
    pub scenario_id: String,
    pub proof_adapter_cargo_allow_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_contract_fields: Vec<String>,
    pub required_discovery_fields: Vec<String>,
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-cargo-allow/parity-boundary-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        parity_contract_path(root),
        provider_contract_parity_contract_path(root),
    ]
}

pub fn provider_contract_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-cargo-allow/parity-provider-contract-v1.toml")
}

pub fn provider_contract_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![provider_contract_parity_contract_path(root)]
}

pub fn load_provider_contract_parity_contract(
    path: &Path,
) -> Result<ProviderContractParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
