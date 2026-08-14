use std::path::PathBuf;

use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::parity::parity_contract_paths;

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-protocol/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_protocol_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_protocol_module".to_string());
    };
    if module != BoundarySurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match fixture {}",
            BoundarySurface::MODULE_ID,
            module
        ));
    }
    Ok(())
}

#[test]
fn intent_engine_does_not_depend_on_proof_protocol() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-protocol") {
        return Err(
            "intent-engine must not depend on proof-protocol (ADR-0002 forbidden edge)".to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_protocol() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-protocol") {
        return Err("cargo-allow must not depend on proof-protocol".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-protocol/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let allowed = fixture
        .get("allowed_upstream_crates")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "parity fixture missing allowed_upstream_crates".to_string())?;
    for crate_name in ALLOWED_UPSTREAM_CRATES {
        if !allowed
            .iter()
            .any(|entry| entry.as_str() == Some(crate_name))
        {
            return Err(format!(
                "fixture missing allowed upstream crate {crate_name}"
            ));
        }
    }
    for edge in FORBIDDEN_DEPENDENCY_EDGES {
        let forbidden = fixture
            .get("forbidden_dependency_edges")
            .and_then(|value| value.as_array())
            .ok_or_else(|| "parity fixture missing forbidden_dependency_edges".to_string())?;
        if !forbidden.iter().any(|entry| entry.as_str() == Some(edge)) {
            return Err(format!("fixture missing forbidden edge {edge}"));
        }
    }
    if upstream_surface_markers().is_empty() {
        return Err("upstream surface markers must not be empty".to_string());
    }
    Ok(())
}

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    Ok(())
}

#[test]
fn validate_proof_plan_rejects_empty_commands() -> Result<(), String> {
    let plan = crate::ProofPlanV1::new("plan-empty", Vec::new());
    match crate::validate_proof_plan(&plan) {
        Err(crate::ProofPlanError::EmptyCommands) => Ok(()),
        other => Err(format!("expected empty_commands, got {other:?}")),
    }
}

#[test]
fn receipt_set_requires_repo_protocol_schema() -> Result<(), String> {
    let set = crate::ProofReceiptSetV1::new(
        "plan-1",
        vec![crate::ProofReceiptBindingV1 {
            binding_id: "binding-1".to_string(),
            plan_id: "plan-1".to_string(),
            command_index: 0,
            analysis_receipt_schema_id: "wrong.schema".to_string(),
            receipt_digest: "sha256:v1:abc".to_string(),
        }],
    );
    match crate::validate_receipt_set(&set) {
        Err(crate::ProofReceiptError::SchemaDrift { .. }) => Ok(()),
        other => Err(format!("expected schema drift, got {other:?}")),
    }
}

#[test]
fn protocol_dtos_round_trip_without_engine_source() -> Result<(), String> {
    // The data seam must round-trip independently of proof-engine: this
    // crate has no engine/intent/application dependency, so serialization
    // here cannot reach semantic evaluation (#2943 step 6).
    let plan = crate::ProofPlanV1::new(
        "plan-roundtrip",
        vec![crate::ProofPlanCommandV1::new(
            "cargo-allow",
            vec!["check".to_string()],
        )],
    );
    crate::validate_proof_plan(&plan).map_err(|err| format!("{err:?}"))?;
    let plan_toml = toml::to_string(&plan).map_err(|err| format!("serialize plan: {err}"))?;
    let reloaded =
        crate::load_proof_plan_toml(&plan_toml).map_err(|err| format!("reload plan: {err}"))?;
    if reloaded != plan {
        return Err("proof plan TOML round-trip drift".to_string());
    }

    let set = crate::ProofReceiptSetV1::new(
        "plan-roundtrip",
        vec![crate::ProofReceiptBindingV1 {
            binding_id: "binding-1".to_string(),
            plan_id: "plan-roundtrip".to_string(),
            command_index: 0,
            analysis_receipt_schema_id: "repo.analysis-receipt.v1".to_string(),
            receipt_digest: "sha256:v1:abc".to_string(),
        }],
    );
    crate::validate_receipt_set(&set).map_err(|err| format!("{err:?}"))?;
    let set_toml = toml::to_string(&set).map_err(|err| format!("serialize receipt set: {err}"))?;
    let reloaded: crate::ProofReceiptSetV1 =
        toml::from_str(&set_toml).map_err(|err| format!("reload receipt set: {err}"))?;
    if reloaded != set {
        return Err("receipt set round-trip drift".to_string());
    }
    Ok(())
}

#[test]
fn protocol_crate_declares_no_semantic_or_application_dependency() -> Result<(), String> {
    // Independence proof for the data seam: no engine, intent, or
    // application crate may appear in any dependency section, so protocol
    // DTOs stay usable with proof-engine source unavailable (#2943).
    let manifest = workspace_root().join("crates/proof-protocol/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read proof-protocol manifest: {err}"))?;
    let table = toml::from_str::<toml::Table>(&text)
        .map_err(|err| format!("parse proof-protocol manifest: {err}"))?;
    let mut declared: Vec<&str> = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = table.get(section).and_then(|value| value.as_table()) else {
            continue;
        };
        declared.extend(deps.keys().map(String::as_str));
    }
    for forbidden in [
        "proof-orchestrator",
        "intent-protocol",
        "intent-compiler",
        "intent-model",
        "cargo-allow",
        "allow-core",
    ] {
        if declared.contains(&forbidden) {
            return Err(format!(
                "proof-protocol must not depend on {forbidden}; it is a data/serialization seam only"
            ));
        }
    }
    Ok(())
}

fn manifest_lists_dependency(manifest_text: &str, crate_name: &str) -> bool {
    let Ok(table) = toml::from_str::<toml::Table>(manifest_text) else {
        return false;
    };
    let Some(deps) = table.get("dependencies").and_then(|value| value.as_table()) else {
        return false;
    };
    deps.contains_key(crate_name)
}

fn manifest_lists_any_dependency(manifest_text: &str, crate_name: &str) -> bool {
    for section in ["dependencies", "dev-dependencies"] {
        let Ok(table) = toml::from_str::<toml::Table>(manifest_text) else {
            continue;
        };
        let Some(deps) = table.get(section).and_then(|value| value.as_table()) else {
            continue;
        };
        if deps.contains_key(crate_name) {
            return true;
        }
    }
    false
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
