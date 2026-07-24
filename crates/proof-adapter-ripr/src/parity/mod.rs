//! Parity fixture discovery for proof-adapter-ripr (#2556).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GripReceiptParityContract {
    pub scenario_id: String,
    pub proof_adapter_ripr_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GripComparisonParityContract {
    pub scenario_id: String,
    pub proof_adapter_ripr_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_comparison_dispositions: Vec<String>,
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-ripr/parity-boundary-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        parity_contract_path(root),
        grip_receipt_parity_contract_path(root),
        grip_comparison_parity_contract_path(root),
    ]
}

pub fn grip_receipt_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-ripr/parity-grip-receipt-v1.toml")
}

pub fn grip_comparison_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-ripr/parity-grip-comparison-v1.toml")
}

pub fn load_grip_receipt_parity_contract(path: &Path) -> Result<GripReceiptParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_grip_comparison_parity_contract(
    path: &Path,
) -> Result<GripComparisonParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
