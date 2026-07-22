use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ParityContract {
    pub scenario_id: String,
    pub allow_diff_module: String,
    pub repo_snapshot_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_transport_fields: Vec<String>,
}

pub fn revision_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![root.join("tests/fixtures/repo-snapshot/parity-committed-head-v1.toml")]
}

pub fn staged_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/repo-snapshot/parity-staged-index-v1.toml")
}

pub fn staged_deletion_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/repo-snapshot/parity-staged-deletion-dirty-replacement-v1.toml")
}

pub fn source_view_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/repo-snapshot/parity-source-view-staged-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = revision_parity_contract_paths(root);
    paths.push(staged_parity_contract_path(root));
    paths.push(staged_deletion_parity_contract_path(root));
    paths.push(source_view_parity_contract_path(root));
    paths
}

pub fn load_parity_contract(path: &Path) -> Result<ParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
