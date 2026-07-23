use crate::phase_obligations::PhaseObligationPlanV1;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PhaseObligationsParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    pub required_obligation_kinds: Vec<String>,
    pub sample_phase: String,
}

pub fn phase_obligations_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-phase-obligations-v1.toml")
}

pub fn phase_obligations_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![phase_obligations_parity_contract_path(root)]
}

pub fn precommit_obligation_plan_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/precommit-obligation-plan-v1.toml")
}

pub fn load_phase_obligations_parity_contract(
    path: &Path,
) -> Result<PhaseObligationsParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_precommit_obligation_plan_fixture(
    root: &Path,
) -> Result<PhaseObligationPlanV1, String> {
    let path = precommit_obligation_plan_fixture_path(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    crate::phase_obligations::load_phase_obligation_plan_toml(&text)
}
