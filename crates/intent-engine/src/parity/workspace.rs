use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkspaceCompositionParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    pub required_composition_fields: Vec<String>,
}

pub fn workspace_composition_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-workspace-composition-v1.toml")
}

pub fn workspace_composition_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![workspace_composition_parity_contract_path(root)]
}

pub fn self_hosted_workspace_composition_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/self-hosted-workspace-composition-v1.toml")
}

pub fn load_workspace_composition_parity_contract(
    path: &Path,
) -> Result<WorkspaceCompositionParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_self_hosted_workspace_composition_fixture(
    root: &Path,
) -> Result<crate::workspace::WorkspaceCompositionV1, String> {
    let path = self_hosted_workspace_composition_fixture_path(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    crate::workspace::load_workspace_composition_toml(&text)
}
