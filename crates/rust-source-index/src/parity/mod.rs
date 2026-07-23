use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TestSubjectsParityContract {
    pub scenario_id: String,
    pub allow_rust_module: String,
    pub rust_source_index_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_subject_fields: Vec<String>,
}

pub fn test_subjects_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/rust-source-index/parity-test-subjects-v1.toml")
}

pub fn test_subjects_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![test_subjects_parity_contract_path(root)]
}

pub fn load_test_subjects_parity_contract(
    path: &Path,
) -> Result<TestSubjectsParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
