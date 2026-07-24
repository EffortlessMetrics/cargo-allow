//! Parity fixture discovery for proof-protocol (#2588-A / #2588-B).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PlanDtosParityContract {
    pub scenario_id: String,
    pub proof_protocol_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_command_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CapabilityDtosParityContract {
    pub scenario_id: String,
    pub proof_protocol_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_capability_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReceiptDtosParityContract {
    pub scenario_id: String,
    pub proof_protocol_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_binding_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ContradictionDtosParityContract {
    pub scenario_id: String,
    pub proof_protocol_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_contradiction_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PhaseGateDtosParityContract {
    pub scenario_id: String,
    pub proof_protocol_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_gate_fields: Vec<String>,
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-boundary-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = plan_dtos_parity_contract_paths(root);
    paths.extend(capability_dtos_parity_contract_paths(root));
    paths.extend(receipt_dtos_parity_contract_paths(root));
    paths.extend(contradiction_dtos_parity_contract_paths(root));
    paths.extend(phase_gate_dtos_parity_contract_paths(root));
    paths.insert(0, parity_contract_path(root));
    paths
}

pub fn plan_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-plan-dtos-v1.toml")
}

pub fn plan_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![plan_dtos_parity_contract_path(root)]
}

pub fn load_plan_dtos_parity_contract(path: &Path) -> Result<PlanDtosParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn capability_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-capability-dtos-v1.toml")
}

pub fn capability_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![capability_dtos_parity_contract_path(root)]
}

pub fn load_capability_dtos_parity_contract(
    path: &Path,
) -> Result<CapabilityDtosParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn receipt_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-receipt-dtos-v1.toml")
}

pub fn receipt_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![receipt_dtos_parity_contract_path(root)]
}

pub fn load_receipt_dtos_parity_contract(path: &Path) -> Result<ReceiptDtosParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn contradiction_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-contradiction-dtos-v1.toml")
}

pub fn contradiction_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![contradiction_dtos_parity_contract_path(root)]
}

pub fn load_contradiction_dtos_parity_contract(
    path: &Path,
) -> Result<ContradictionDtosParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn phase_gate_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-phase-gate-dtos-v1.toml")
}

pub fn phase_gate_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![phase_gate_dtos_parity_contract_path(root)]
}

pub fn load_phase_gate_dtos_parity_contract(
    path: &Path,
) -> Result<PhaseGateDtosParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
