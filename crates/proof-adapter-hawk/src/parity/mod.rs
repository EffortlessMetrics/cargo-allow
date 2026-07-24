//! Parity fixture discovery for proof-adapter-hawk (#2555).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AnalysisReceiptParityContract {
    pub scenario_id: String,
    pub proof_adapter_hawk_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FindingMappingParityContract {
    pub scenario_id: String,
    pub proof_adapter_hawk_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_result_classes: Vec<String>,
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-hawk/parity-boundary-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        parity_contract_path(root),
        analysis_receipt_parity_contract_path(root),
        finding_mapping_parity_contract_path(root),
    ]
}

pub fn analysis_receipt_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-hawk/parity-analysis-receipt-v1.toml")
}

pub fn finding_mapping_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-adapter-hawk/parity-finding-mapping-v1.toml")
}

pub fn load_analysis_receipt_parity_contract(
    path: &Path,
) -> Result<AnalysisReceiptParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_finding_mapping_parity_contract(
    path: &Path,
) -> Result<FindingMappingParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
