use std::path::PathBuf;

use super::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use super::conformance::run_fake_provider_conformance;
use super::contracts::validate_provider_surface;
use super::fake_provider::FakeProofProviderV1;
use super::parity::parity_contract_paths;

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-provider-api/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_provider_api_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_provider_api_module".to_string());
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
fn intent_engine_does_not_depend_on_proof_provider_api() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-provider-api") {
        return Err(
            "intent-engine must not depend on proof-provider-api (ADR-0002 forbidden edge)"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_provider_api() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-provider-api") {
        return Err("cargo-allow must not depend on proof-provider-api".to_string());
    }
    Ok(())
}

#[test]
fn fake_provider_passes_conformance_harness() -> Result<(), String> {
    run_fake_provider_conformance()
}

#[test]
fn validate_provider_surface_accepts_fake_provider() -> Result<(), String> {
    let provider = FakeProofProviderV1::new();
    validate_provider_surface(&provider).map_err(|err| err.as_str().to_string())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-provider-api/parity-boundary-v1.toml");
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
