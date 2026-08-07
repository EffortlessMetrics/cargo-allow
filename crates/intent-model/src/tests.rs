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

const SPEC_SYSTEM_DTO_FILES: &[&str] = &[
    "active_goal.rs",
    "authored_mapping.rs",
    "compiled_graph.rs",
    "config.rs",
    "doc_artifacts.rs",
    "implementation_slice.rs",
    "import_roots.rs",
    "precommit.rs",
    "requirement.rs",
    "support_tiers.rs",
];

#[test]
fn spec_system_snapshot_matches_intent_model() -> Result<(), String> {
    let root = workspace_root();
    for file in SPEC_SYSTEM_DTO_FILES {
        let canonical = std::fs::read_to_string(
            root.join(format!("crates/intent-model/src/spec_system/{file}")),
        )
        .map_err(|err| format!("read canonical spec_system/{file}: {err}"))?;
        let packaged = std::fs::read_to_string(root.join(format!(
            "crates/allow-policy/src/snapshot_package/spec_system/{file}"
        )))
        .map_err(|err| format!("read allow-policy snapshot spec_system/{file}: {err}"))?;
        if canonical.replace("\r\n", "\n") != packaged.replace("\r\n", "\n") {
            return Err(format!(
                "allow-policy snapshot_package/spec_system/{file} must match intent-model spec_system/{file}"
            ));
        }
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
