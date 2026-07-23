use crate::SpecSystemSurface;
use crate::parity::{SpecSystemParityContract, load_spec_system_parity_contract};
use std::path::PathBuf;

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::spec_system_parity_contract_paths(&root) {
        let contract = load_spec_system_parity_contract(&path)?;
        validate_contract(&contract)?;
    }
    Ok(())
}

#[test]
fn spec_system_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = crate::parity::spec_system_parity_contract_path(&root);
    let contract = load_spec_system_parity_contract(&contract_path)?;
    if contract.intent_model_module != SpecSystemSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            SpecSystemSurface::MODULE_ID,
            contract.intent_model_module
        ));
    }
    if contract.allow_policy_module != "allow-policy::spec_system" {
        return Err("fixture must reference allow-policy::spec_system".to_string());
    }
    if contract.parity_case != "parity-intent-model-spec-system-v1" {
        return Err("fixture parity_case mismatch".to_string());
    }
    Ok(())
}

fn validate_contract(contract: &SpecSystemParityContract) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-allow-policy-spec-system" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_config_fields.len() < 3 {
        return Err("required_config_fields too small".to_string());
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
