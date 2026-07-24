//! Parity fixture discovery for intent-edit (#2613-A / #2613-B / #2613-C).

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DialectAdapterParityContract {
    pub scenario_id: String,
    pub intent_edit_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub canonical_dialect_ids: Vec<String>,
    pub required_normalization_behaviors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApprovalCurrentnessParityContract {
    pub scenario_id: String,
    pub intent_edit_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_envelope_fields: Vec<String>,
    pub fail_closed_states: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RepoEditTranslationParityContract {
    pub scenario_id: String,
    pub intent_edit_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub required_request_fields: Vec<String>,
    pub supported_apply_modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RecompileContractParityContract {
    pub scenario_id: String,
    pub intent_edit_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub target_transport_schema_id: String,
    pub required_obligation_fields: Vec<String>,
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = edit_plan_parity_contract_paths(root);
    paths.extend(dialect_adapter_parity_contract_paths(root));
    paths.extend(approval_currentness_parity_contract_paths(root));
    paths.extend(repo_edit_translation_parity_contract_paths(root));
    paths.extend(recompile_contract_parity_contract_paths(root));
    paths.insert(0, parity_contract_path(root));
    paths
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-boundary-v1.toml")
}

pub fn edit_plan_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-edit-plan-v1.toml")
}

pub fn edit_plan_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![edit_plan_parity_contract_path(root)]
}

pub fn dialect_adapter_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-dialect-adapter-v1.toml")
}

pub fn dialect_adapter_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![dialect_adapter_parity_contract_path(root)]
}

pub fn approval_currentness_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-approval-currentness-v1.toml")
}

pub fn approval_currentness_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![approval_currentness_parity_contract_path(root)]
}

pub fn load_edit_plan_parity_contract(path: &Path) -> Result<EditPlanParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_dialect_adapter_parity_contract(
    path: &Path,
) -> Result<DialectAdapterParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_approval_currentness_parity_contract(
    path: &Path,
) -> Result<ApprovalCurrentnessParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn repo_edit_translation_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-repo-edit-translation-v1.toml")
}

pub fn repo_edit_translation_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![repo_edit_translation_parity_contract_path(root)]
}

pub fn load_repo_edit_translation_parity_contract(
    path: &Path,
) -> Result<RepoEditTranslationParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn recompile_contract_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-edit/parity-recompile-contract-v1.toml")
}

pub fn recompile_contract_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![recompile_contract_parity_contract_path(root)]
}

pub fn load_recompile_contract_parity_contract(
    path: &Path,
) -> Result<RecompileContractParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
