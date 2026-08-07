use std::path::PathBuf;

use proof_engine::{run_provider_conformance, validate_provider_plan, validate_provider_surface};

use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::grip_comparison::{
    GripComparisonDispositionV1, RequirementEvidencePurposeV1, compare_requirement_grip,
};
use crate::grip_comparison_surface::GripComparisonSurface;
use crate::grip_receipt::{
    RiprCompletenessV1, RiprExecutionModeV1, RiprGripDispositionV1, RiprGripReceiptV1,
    validate_ripr_grip_receipt,
};
use crate::grip_receipt_surface::GripReceiptSurface;
use crate::parity::{
    grip_comparison_parity_contract_path, grip_receipt_parity_contract_path,
    load_grip_comparison_parity_contract, load_grip_receipt_parity_contract, parity_contract_paths,
};
use crate::receipt_currentness::{
    RiprCurrentnessRequest, RiprReceiptCurrentnessStatusV1, evaluate_receipt_currentness,
};
use crate::receipt_currentness_surface::ReceiptCurrentnessSurface;
use crate::ripr_adapter::RiprProofProviderV1;
use crate::ripr_adapter_surface::RiprAdapterSurface;

fn strong_grip_receipt() -> RiprGripReceiptV1 {
    RiprGripReceiptV1 {
        schema_id: crate::grip_receipt::RIPR_GRIP_RECEIPT_SCHEMA_ID.to_string(),
        receipt_id: "ripr-grip-strong-v1".to_string(),
        ripr_provider_id: "ripr.test-provider.v1".to_string(),
        ripr_schema_generation: "test-grip-summary.v2".to_string(),
        analyzer_generation: "ripr-analyzer.v1".to_string(),
        config_fingerprint: "sha256:v1:config".to_string(),
        snapshot_digest: "sha256:v1:snapshot".to_string(),
        subject_ref: "subject/runtime-promotion".to_string(),
        seam_ref: "seam/self-hosted-runtime-promotion".to_string(),
        requirement_id: "spec-only-runtime-promotion".to_string(),
        execution_mode: RiprExecutionModeV1::CapturedReceipt,
        completeness: RiprCompletenessV1::Complete,
        grip_disposition: RiprGripDispositionV1::LikelyDiscriminating,
        receipt_digest: "sha256:v1:receipt".to_string(),
    }
}

fn evidence_purpose() -> RequirementEvidencePurposeV1 {
    RequirementEvidencePurposeV1 {
        purpose_id: "purpose/runtime-promotion-v1".to_string(),
        requirement_id: "spec-only-runtime-promotion".to_string(),
        seam_ref: "seam/self-hosted-runtime-promotion".to_string(),
        subject_ref: "subject/runtime-promotion".to_string(),
        expected_discriminators: vec!["reject_spec_only_implemented_claim".to_string()],
    }
}

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-adapter-ripr/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_adapter_ripr_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_adapter_ripr_module".to_string());
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
fn grip_receipt_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract = load_grip_receipt_parity_contract(&grip_receipt_parity_contract_path(&root))?;
    if contract.proof_adapter_ripr_module != GripReceiptSurface::MODULE_ID {
        return Err("grip receipt surface drift".to_string());
    }
    Ok(())
}

#[test]
fn grip_comparison_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract =
        load_grip_comparison_parity_contract(&grip_comparison_parity_contract_path(&root))?;
    if contract.proof_adapter_ripr_module != GripComparisonSurface::MODULE_ID {
        return Err("grip comparison surface drift".to_string());
    }
    Ok(())
}

#[test]
fn validate_strong_and_weak_grip_receipts() -> Result<(), String> {
    let strong = strong_grip_receipt();
    validate_ripr_grip_receipt(&strong).map_err(|err| err.as_str())?;
    let mut weak = strong.clone();
    weak.receipt_id = "ripr-grip-weak-v1".to_string();
    weak.grip_disposition = RiprGripDispositionV1::LikelyRelevantWithLimitations;
    validate_ripr_grip_receipt(&weak).map_err(|err| err.as_str())?;
    Ok(())
}

#[test]
fn currentness_rejects_stale_snapshot() -> Result<(), String> {
    let receipt = strong_grip_receipt();
    let report = evaluate_receipt_currentness(&RiprCurrentnessRequest {
        receipt: &receipt,
        expected_snapshot_digest: "sha256:v1:other",
        expected_subject_ref: receipt.subject_ref.as_str(),
        expected_seam_ref: receipt.seam_ref.as_str(),
        expected_requirement_id: receipt.requirement_id.as_str(),
    });
    if report.status != RiprReceiptCurrentnessStatusV1::StaleSnapshot {
        return Err("expected stale snapshot".to_string());
    }
    Ok(())
}

#[test]
fn compare_requirement_grip_preserves_strong_disposition() -> Result<(), String> {
    let receipt = strong_grip_receipt();
    let purpose = evidence_purpose();
    let comparison = compare_requirement_grip(&crate::grip_comparison::GripComparisonRequest {
        purpose: &purpose,
        receipt: &receipt,
        expected_snapshot_digest: receipt.snapshot_digest.as_str(),
    })
    .map_err(|err| err.as_str())?;
    if comparison.disposition != GripComparisonDispositionV1::LikelyDiscriminating {
        return Err("expected likely discriminating comparison".to_string());
    }
    Ok(())
}

#[test]
fn compare_requirement_grip_marks_stale_summary() -> Result<(), String> {
    let receipt = strong_grip_receipt();
    let purpose = evidence_purpose();
    let comparison = compare_requirement_grip(&crate::grip_comparison::GripComparisonRequest {
        purpose: &purpose,
        receipt: &receipt,
        expected_snapshot_digest: "sha256:v1:stale",
    })
    .map_err(|err| err.as_str())?;
    if comparison.disposition != GripComparisonDispositionV1::StaleOrInvalidSummary {
        return Err("expected stale comparison disposition".to_string());
    }
    Ok(())
}

#[test]
fn ripr_provider_conformance_passes() -> Result<(), String> {
    let provider = RiprProofProviderV1::new();
    run_provider_conformance(&provider)
}

#[test]
fn intent_engine_does_not_depend_on_proof_adapter_ripr() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-adapter-ripr") {
        return Err(
            "intent-engine must not depend on proof-adapter-ripr (ADR-0002 forbidden edge)"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_adapter_ripr() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-adapter-ripr") {
        return Err("cargo-allow must not depend on proof-adapter-ripr".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-adapter-ripr/parity-boundary-v1.toml");
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
        GripReceiptSurface::MODULE_ID,
        ReceiptCurrentnessSurface::MODULE_ID,
        GripComparisonSurface::MODULE_ID,
        RiprAdapterSurface::MODULE_ID,
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
    let provider = RiprProofProviderV1::new();
    validate_provider_surface(&provider).map_err(|err| err.as_str())?;
    let plan = proof_protocol::ProofPlanV1::new(
        "proof-adapter-ripr-plan-v1",
        vec![proof_protocol::ProofPlanCommandV1::new(
            "ripr",
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
