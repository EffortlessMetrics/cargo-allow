use std::path::PathBuf;

use proof_engine::{
    ProofProviderV1, run_provider_conformance, validate_provider_plan, validate_provider_surface,
};
use proof_protocol::ProofPlanCommandV1;
use proof_protocol::ProofPlanV1;

use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::cargo_allow_provider::CargoAllowProofProviderV1;
use crate::cargo_allow_provider_surface::CargoAllowProviderSurface;
use crate::parity::{
    load_provider_contract_parity_contract, parity_contract_paths,
    provider_contract_parity_contract_paths,
};
use crate::process_protocol::{compile_cargo_allow_dry_run, validate_process_protocol_plan};
use crate::process_protocol_surface::ProcessProtocolSurface;
use crate::provider_contract::{default_cargo_allow_provider_contract, validate_provider_contract};
use crate::provider_contract_surface::ProviderContractSurface;
use crate::provider_discovery_surface::ProviderDiscoverySurface;

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path =
        root.join("tests/fixtures/proof-adapter-cargo-allow/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_adapter_cargo_allow_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_adapter_cargo_allow_module".to_string());
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
fn provider_contract_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = provider_contract_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing provider contract parity fixture path".to_string())?;
    let contract = load_provider_contract_parity_contract(&contract_path)?;
    if contract.proof_adapter_cargo_allow_module != ProviderContractSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            ProviderContractSurface::MODULE_ID,
            contract.proof_adapter_cargo_allow_module
        ));
    }
    Ok(())
}

#[test]
fn default_provider_contract_is_snapshot_bound_read_only() -> Result<(), String> {
    let contract = default_cargo_allow_provider_contract();
    validate_provider_contract(&contract).map_err(|err| err.as_str())?;
    if !contract.snapshot_bound {
        return Err("contract must be snapshot_bound".to_string());
    }
    Ok(())
}

#[test]
fn cargo_allow_provider_conformance_passes() -> Result<(), String> {
    let provider = CargoAllowProofProviderV1::new();
    run_provider_conformance(&provider)
}

#[test]
fn process_protocol_compiles_no_new_dry_run() -> Result<(), String> {
    let plan = ProofPlanV1::new(
        "proof-adapter-cargo-allow-dry-run-v1",
        vec![ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        )],
    );
    validate_process_protocol_plan(&plan).map_err(|err| err.as_str())?;
    let reports = compile_cargo_allow_dry_run(&plan).map_err(|err| err.as_str())?;
    let first = reports
        .first()
        .ok_or_else(|| "expected one dry-run report".to_string())?;
    if first.program != "cargo-allow" {
        return Err("unexpected program in dry-run report".to_string());
    }
    Ok(())
}

#[test]
fn provider_advertises_capability_report_without_mutation() -> Result<(), String> {
    let provider = CargoAllowProofProviderV1::new();
    let capability = provider
        .capability_catalog()
        .capabilities
        .iter()
        .find(|capability| capability.capability_id == "cargo-allow.capabilities.json")
        .ok_or_else(|| "capability report was not advertised".to_string())?;
    if capability.kind != proof_protocol::ProofCapabilityKindV1::StaticReport
        || !capability.statement.contains("sensor-capabilities.v1")
    {
        return Err("capability report has the wrong provider projection".to_string());
    }

    let plan = ProofPlanV1::new(
        "proof-adapter-cargo-allow-capabilities-v1",
        vec![ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "capabilities".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
        )],
    );
    let report = compile_cargo_allow_dry_run(&plan)
        .map_err(|err| err.as_str())?
        .pop()
        .ok_or_else(|| "capability report dry-run was empty".to_string())?;
    if !report.would_read.is_empty()
        || !report.would_write.is_empty()
        || report.network != proof_engine::NetworkAccessV1::None
    {
        return Err("capability report dry-run was not read-only".to_string());
    }
    Ok(())
}

#[test]
fn capability_report_rejects_output_tail() -> Result<(), String> {
    let plan = ProofPlanV1::new(
        "proof-adapter-cargo-allow-capabilities-output-v1",
        vec![ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "capabilities".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--output".to_string(),
                "report.json".to_string(),
            ],
        )],
    );
    match compile_cargo_allow_dry_run(&plan) {
        Err(crate::process_protocol::ProcessProtocolError::UnsupportedCommand { .. }) => Ok(()),
        Ok(_) => Err("capability report accepted a mutating output tail".to_string()),
        Err(error) => Err(format!(
            "unexpected capability report error: {}",
            error.as_str()
        )),
    }
}

#[test]
fn intent_engine_does_not_depend_on_proof_adapter_cargo_allow() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-adapter-cargo-allow") {
        return Err(
            "intent-engine must not depend on proof-adapter-cargo-allow (ADR-0002 forbidden edge)"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_adapter_cargo_allow() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-adapter-cargo-allow") {
        return Err("cargo-allow must not depend on proof-adapter-cargo-allow".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path =
        root.join("tests/fixtures/proof-adapter-cargo-allow/parity-boundary-v1.toml");
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
fn surface_markers_are_distinct() -> Result<(), String> {
    let markers = [
        BoundarySurface::MODULE_ID,
        ProviderContractSurface::MODULE_ID,
        ProviderDiscoverySurface::MODULE_ID,
        ProcessProtocolSurface::MODULE_ID,
        CargoAllowProviderSurface::MODULE_ID,
    ];
    for (index, left) in markers.iter().enumerate() {
        for right in markers.iter().skip(index + 1) {
            if left == right {
                return Err(format!("duplicate surface marker {left}"));
            }
        }
    }
    Ok(())
}

#[test]
fn validate_provider_plan_wires_process_protocol() -> Result<(), String> {
    let provider = CargoAllowProofProviderV1::new();
    validate_provider_surface(&provider).map_err(|err| err.as_str())?;
    let plan = ProofPlanV1::new(
        "proof-adapter-cargo-allow-plan-v1",
        vec![ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        )],
    );
    validate_provider_plan(&provider, &plan).map_err(|err| err.as_str())?;
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
