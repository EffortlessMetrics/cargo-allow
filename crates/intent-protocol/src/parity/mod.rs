use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IdentityQueryParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_protocol_module: String,
    pub required_identity_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ViewDiffClosureParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_protocol_module: String,
    pub required_view_fields: Vec<String>,
    pub required_closure_fields: Vec<String>,
}

pub fn identity_query_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-protocol/parity-identity-query-v1.toml")
}

pub fn identity_query_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![identity_query_parity_contract_path(root)]
}

pub fn view_diff_closure_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-protocol/parity-view-diff-closure-v1.toml")
}

pub fn view_diff_closure_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![view_diff_closure_parity_contract_path(root)]
}

pub fn load_identity_query_parity_contract(
    path: &Path,
) -> Result<IdentityQueryParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_view_diff_closure_parity_contract(
    path: &Path,
) -> Result<ViewDiffClosureParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
