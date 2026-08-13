use crate::parity::{SpecSystemParityContract, load_spec_system_parity_contract};
use crate::{IntentModelError, IntentModelErrorKind};
use std::path::{Path, PathBuf};

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::spec_system_parity_contract_paths(&root) {
        let contract = load_spec_system_parity_contract(&path)?;
        validate_contract(&contract)?;
    }
    Ok(())
}

// authored_mapping.rs is intentionally different between the two crates:
// intent-model inlines shared ID types (#3304); allow-policy imports from
// its own compiled_graph.rs snapshot copy. The remaining files still match.
const SPEC_SYSTEM_DTO_FILES: &[&str] = &[
    "active_goal.rs",
    "config.rs",
    "doc_artifacts.rs",
    "implementation_slice.rs",
    "import_roots.rs",
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

#[test]
fn intent_error_retains_kind_and_toml_location() -> Result<(), String> {
    let source = "mode = \"blocking\"\nunknown = true\n";
    let start = source
        .find("unknown")
        .ok_or_else(|| "fixture token missing".to_string())?;
    let error = IntentModelError::with_kind(IntentModelErrorKind::InvalidConfig, "invalid config")
        .with_toml_span(
            Some(Path::new("intent.toml")),
            source,
            Some(start..start + 7),
        );

    if error.kind() != IntentModelErrorKind::InvalidConfig {
        return Err(format!("unexpected error kind: {:?}", error.kind()));
    }
    let location = error
        .location()
        .ok_or_else(|| "TOML location missing".to_string())?;
    if location.path.as_deref() != Some("intent.toml") || location.line != 2 || location.column != 1
    {
        return Err(format!("unexpected TOML location: {location:?}"));
    }
    Ok(())
}

#[test]
fn intent_model_has_no_cargo_allow_product_dependency() -> Result<(), String> {
    let manifest = std::fs::read_to_string(workspace_root().join("crates/intent-model/Cargo.toml"))
        .map_err(|error| format!("read intent-model manifest: {error}"))?;
    let dependencies = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .ok_or_else(|| "intent-model dependencies table missing".to_string())?;
    if dependencies.contains("allow-core") || dependencies.contains("allow_core") {
        return Err("intent-model retains a cargo-allow product dependency".to_string());
    }
    Ok(())
}

#[test]
fn intent_content_identity_remains_compatible() {
    assert_eq!(crate::stable_hash_hex("abc"), "fnv1a64:e71fa2190541574b");
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
