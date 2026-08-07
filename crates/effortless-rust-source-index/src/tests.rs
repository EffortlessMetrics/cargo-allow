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
fn test_subjects_package_copy_matches_rust_source_index() -> Result<(), String> {
    let root = workspace_root();
    let canonical =
        std::fs::read_to_string(root.join("crates/rust-source-index/src/test_subjects.rs"))
            .map_err(|err| format!("read canonical test_subjects: {err}"))?;
    let packaged = std::fs::read_to_string(
        root.join("crates/allow-rust/src/snapshot_package/test_subjects.rs"),
    )
    .map_err(|err| format!("read allow-rust snapshot copy: {err}"))?;
    if canonical.replace("\r\n", "\n") != packaged.replace("\r\n", "\n") {
        return Err(
            "allow-rust snapshot_package/test_subjects.rs must match rust-source-index test_subjects.rs"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn syntax_package_copy_matches_rust_source_index() -> Result<(), String> {
    let root = workspace_root();
    let canonical = std::fs::read_to_string(root.join("crates/rust-source-index/src/syntax.rs"))
        .map_err(|err| format!("read canonical syntax: {err}"))?;
    let packaged =
        std::fs::read_to_string(root.join("crates/allow-rust/src/snapshot_package/syntax.rs"))
            .map_err(|err| format!("read allow-rust snapshot syntax: {err}"))?;
    if canonical.replace("\r\n", "\n") != packaged.replace("\r\n", "\n") {
        return Err(
            "allow-rust snapshot_package/syntax.rs must match rust-source-index syntax.rs"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn inventory_package_copy_matches_rust_source_index() -> Result<(), String> {
    let root = workspace_root();
    let canonical = std::fs::read_to_string(root.join("crates/rust-source-index/src/inventory.rs"))
        .map_err(|err| format!("read canonical inventory: {err}"))?
        .replace("\r\n", "\n");
    let packaged =
        std::fs::read_to_string(root.join("crates/allow-rust/src/snapshot_package/inventory.rs"))
            .map_err(|err| format!("read allow-rust snapshot inventory: {err}"))?
            .replace("\r\n", "\n");
    let canonical = canonical.replace(
        "use crate::syntax::{node_text, parse_rust_syntax, source_column};\nuse crate::test_subjects::*;",
        "use super::subject_syntax::{node_text, parse_rust_syntax, source_column};\nuse super::subject_types::*;",
    );
    if canonical != packaged {
        return Err(
            "allow-rust snapshot_package/inventory.rs must match rust-source-index inventory.rs (import-adjusted)"
                .to_string(),
        );
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
