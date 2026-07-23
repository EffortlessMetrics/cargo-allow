use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GraphComparisonParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    pub required_movement_kinds: Vec<String>,
}

pub fn graph_comparison_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-graph-comparison-v1.toml")
}

pub fn graph_comparison_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![graph_comparison_parity_contract_path(root)]
}

pub fn graph_movement_kinds_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/graph-movement-kinds-v1.toml")
}

pub fn load_graph_comparison_parity_contract(
    path: &Path,
) -> Result<GraphComparisonParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_graph_movement_kinds_fixture(root: &Path) -> Result<Vec<String>, String> {
    let path = graph_movement_kinds_fixture_path(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))?;
    let Some(kinds) = table.get("movement_kinds").and_then(|value| value.as_array()) else {
        return Err("graph movement kinds fixture missing movement_kinds".to_string());
    };
    kinds
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "movement_kinds entries must be strings".to_string())
        })
        .collect()
}
