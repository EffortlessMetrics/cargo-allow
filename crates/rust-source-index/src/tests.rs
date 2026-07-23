use crate::TestSubjectsSurface;
use crate::parity::{TestSubjectsParityContract, load_test_subjects_parity_contract};
use std::path::PathBuf;

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::test_subjects_parity_contract_paths(&root) {
        let contract = load_test_subjects_parity_contract(&path)?;
        validate_contract(&contract)?;
    }
    Ok(())
}

#[test]
fn test_subjects_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = crate::parity::test_subjects_parity_contract_path(&root);
    let contract = load_test_subjects_parity_contract(&contract_path)?;
    if contract.rust_source_index_module != TestSubjectsSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            TestSubjectsSurface::MODULE_ID,
            contract.rust_source_index_module
        ));
    }
    if contract.allow_rust_module != "allow-rust::test_subjects" {
        return Err("fixture must reference allow-rust::test_subjects".to_string());
    }
    if contract.parity_case != "parity-rust-source-index-test-subjects-v1" {
        return Err("fixture parity_case mismatch".to_string());
    }
    Ok(())
}

fn validate_contract(contract: &TestSubjectsParityContract) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-allow-rust-test-subjects" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_subject_fields.len() < 4 {
        return Err("required_subject_fields too small".to_string());
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
