use std::path::PathBuf;

use proof_engine::{run_provider_conformance, validate_provider_plan, validate_provider_surface};

use crate::analysis_receipt::{
    HawkAnalysisReceiptV1, HawkExecutionModeV1, HawkFindingV1, validate_hawk_analysis_receipt,
};
use crate::analysis_receipt_surface::AnalysisReceiptSurface;
use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::finding_mapping::{HawkResultClassV1, map_hawk_finding, map_missing_finding};
use crate::finding_mapping_surface::FindingMappingSurface;
use crate::hawk_adapter::HawkProofProviderV1;
use crate::hawk_adapter_surface::HawkAdapterSurface;
use crate::parity::{
    analysis_receipt_parity_contract_path, finding_mapping_parity_contract_path,
    load_analysis_receipt_parity_contract, load_finding_mapping_parity_contract,
    parity_contract_paths,
};
use crate::receipt_currentness::{
    HawkCurrentnessRequest, HawkReceiptCurrentnessStatusV1, evaluate_hawk_receipt_currentness,
};
use crate::receipt_currentness_surface::ReceiptCurrentnessSurface;
use crate::source_anchor_resolution::{
    SourceAnchorRequest, SourceAnchorResolutionClassV1, resolve_source_anchor,
};
use crate::source_anchor_resolution_surface::SourceAnchorResolutionSurface;

fn sample_receipt() -> HawkAnalysisReceiptV1 {
    HawkAnalysisReceiptV1 {
        schema_id: crate::analysis_receipt::HAWK_ANALYSIS_RECEIPT_SCHEMA_ID.to_string(),
        receipt_id: "hawk-analysis-cargo-allow-v1".to_string(),
        hawk_frontend_digest: "sha256:v1:frontend".to_string(),
        hawk_driver_digest: "sha256:v1:driver".to_string(),
        rustc_release: "1.95.0".to_string(),
        rustc_commit: "abc123".to_string(),
        host_triple: "x86_64-unknown-linux-gnu".to_string(),
        hawk_schema_generation: "hawk-report.v1".to_string(),
        config_path: "proof/hawk/cargo-allow.toml".to_string(),
        config_digest: "sha256:v1:config".to_string(),
        manifest_digest: "sha256:v1:manifest".to_string(),
        lockfile_digest: "sha256:v1:lock".to_string(),
        feature_profile: "default".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        snapshot_digest: "sha256:v1:snapshot".to_string(),
        product_name: "cargo-allow".to_string(),
        raw_payload_digest: "sha256:v1:payload".to_string(),
        execution_mode: HawkExecutionModeV1::CapturedReport,
        findings: vec![
            HawkFindingV1 {
                hawk_code: "hawk::unnecessary_public".to_string(),
                declaration_identity: "cargo_allow::identity::PRODUCT_NAME".to_string(),
                test_only: Some(false),
            },
            HawkFindingV1 {
                hawk_code: "hawk::dead_public".to_string(),
                declaration_identity: "cargo_allow::legacy::dead_helper".to_string(),
                test_only: None,
            },
        ],
    }
}

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-adapter-hawk/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_adapter_hawk_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_adapter_hawk_module".to_string());
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
fn analysis_receipt_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract =
        load_analysis_receipt_parity_contract(&analysis_receipt_parity_contract_path(&root))?;
    if contract.proof_adapter_hawk_module != AnalysisReceiptSurface::MODULE_ID {
        return Err("analysis receipt surface drift".to_string());
    }
    Ok(())
}

#[test]
fn finding_mapping_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract =
        load_finding_mapping_parity_contract(&finding_mapping_parity_contract_path(&root))?;
    if contract.proof_adapter_hawk_module != FindingMappingSurface::MODULE_ID {
        return Err("finding mapping surface drift".to_string());
    }
    Ok(())
}

#[test]
fn validate_captured_hawk_report() -> Result<(), String> {
    validate_hawk_analysis_receipt(&sample_receipt()).map_err(|err| err.as_str())?;
    Ok(())
}

#[test]
fn map_unnecessary_public_production_live() -> Result<(), String> {
    let finding = HawkFindingV1 {
        hawk_code: "hawk::unnecessary_public".to_string(),
        declaration_identity: "cargo_allow::identity::PRODUCT_NAME".to_string(),
        test_only: Some(false),
    };
    let mapped = map_hawk_finding(&finding);
    if mapped.primary_class != HawkResultClassV1::ProductionLiveFromConfiguredClosure {
        return Err("expected production live mapping".to_string());
    }
    Ok(())
}

#[test]
fn map_missing_diagnostic_as_not_proven() -> Result<(), String> {
    let mapped = map_missing_finding("cargo_allow::missing::symbol");
    if mapped.primary_class != HawkResultClassV1::NoFindingObserved {
        return Err("expected no finding observed".to_string());
    }
    if mapped.secondary_class != Some(HawkResultClassV1::NotProven) {
        return Err("expected not proven secondary class".to_string());
    }
    Ok(())
}

#[test]
fn resolve_source_anchor_exact_match() -> Result<(), String> {
    let receipt = sample_receipt();
    let resolution = resolve_source_anchor(&SourceAnchorRequest {
        receipt: &receipt,
        requested_anchor: "cargo_allow::identity::PRODUCT_NAME",
        expected_product_name: "cargo-allow",
    })
    .map_err(|err| err.as_str())?;
    if resolution.resolution != SourceAnchorResolutionClassV1::Exact {
        return Err("expected exact resolution".to_string());
    }
    Ok(())
}

#[test]
fn currentness_rejects_stale_toolchain() -> Result<(), String> {
    let receipt = sample_receipt();
    let report = evaluate_hawk_receipt_currentness(&HawkCurrentnessRequest {
        receipt: &receipt,
        expected_snapshot_digest: receipt.snapshot_digest.as_str(),
        expected_config_digest: receipt.config_digest.as_str(),
        expected_rustc_release: "1.94.0",
        expected_target_triple: receipt.target_triple.as_str(),
    });
    if report.status != HawkReceiptCurrentnessStatusV1::StaleToolchain {
        return Err("expected stale toolchain".to_string());
    }
    Ok(())
}

#[test]
fn hawk_provider_conformance_passes() -> Result<(), String> {
    run_provider_conformance(&HawkProofProviderV1::new())
}

#[test]
fn intent_engine_does_not_depend_on_proof_adapter_hawk() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-adapter-hawk") {
        return Err(
            "intent-engine must not depend on proof-adapter-hawk (ADR-0002 forbidden edge)"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_adapter_hawk() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-adapter-hawk") {
        return Err("cargo-allow must not depend on proof-adapter-hawk".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-adapter-hawk/parity-boundary-v1.toml");
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
        AnalysisReceiptSurface::MODULE_ID,
        FindingMappingSurface::MODULE_ID,
        SourceAnchorResolutionSurface::MODULE_ID,
        ReceiptCurrentnessSurface::MODULE_ID,
        HawkAdapterSurface::MODULE_ID,
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
fn validate_provider_plan_accepts_non_empty_plan() -> Result<(), String> {
    let provider = HawkProofProviderV1::new();
    validate_provider_surface(&provider).map_err(|err| err.as_str())?;
    let plan = proof_protocol::ProofPlanV1::new(
        "proof-adapter-hawk-plan-v1",
        vec![proof_protocol::ProofPlanCommandV1::new(
            "cargo-hawk",
            vec!["validate".to_string()],
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
