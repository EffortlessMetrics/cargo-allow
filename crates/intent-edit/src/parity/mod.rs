//! Parity fixture discovery for intent-edit (#2613-A / #2613-B).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EditPlanParityContract {
    pub scenario_id: String,
    pub intent_edit_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_action_fields: Vec<String>,
    pub required_resolution_strategies: Vec<String>,
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("tests/fixtures/intent-edit/parity-boundary-v1.toml"),
        root.join("tests/fixtures/intent-edit/parity-edit-plan-v1.toml"),
    ]
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    parity_contract_paths(root)
        .into_iter()
        .next()
        .unwrap_or_else(|| root.join("tests/fixtures/intent-edit/parity-boundary-v1.toml"))
}

pub fn edit_plan_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-edit-plan-v1.toml")
}

pub fn edit_plan_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![edit_plan_parity_contract_path(root)]
}

pub fn load_edit_plan_parity_contract(path: &Path) -> Result<EditPlanParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
