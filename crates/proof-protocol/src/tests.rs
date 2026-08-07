use std::path::PathBuf;

use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::parity::{
    load_proof_corpus_contract, load_proof_corpus_fixture, parity_contract_paths,
    proof_corpus_contract_path,
};

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
fn proof_corpus_contract_matches_external_profile() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = proof_corpus_contract_path(&root);
    let contract = load_proof_corpus_contract(&contract_path)?;
    if contract.profile_id != crate::RIPR_EXTERNAL_PROOF_PROFILE_ID {
        return Err("contract profile_id mismatch".to_string());
    }
    Ok(())
}

#[test]
fn proof_corpus_fixture_records_all_dimensions_and_states() -> Result<(), String> {
    let root = workspace_root();
    let corpus = load_proof_corpus_fixture(&root)?;
    if corpus.corpus_digest != crate::PROOF_CORPUS_DIGEST_V1 {
        return Err("corpus digest drift".to_string());
    }
    for state in crate::canonical_proof_result_states() {
        if !state.allows_passed_composition() && state.is_non_execution() {
            crate::validate_composition_honesty(*state)?;
        }
    }
    Ok(())
}

#[test]
fn composition_honesty_rejects_non_execution_passed_upgrade() -> Result<(), String> {
    let aggregate = crate::compose_blocking_aggregate(&[
        crate::ProofResultStateV1::ProofPassed,
        crate::ProofResultStateV1::ProviderUnavailable,
    ]);
    if aggregate == crate::ProofResultStateV1::ProofPassed {
        return Err("non-execution must not compose to proof_passed".to_string());
    }
    Ok(())
}

#[test]
fn binding_currentness_distinguishes_missing_stale_and_incomparable() -> Result<(), String> {
    let expected = crate::ProofBindingIdentityV1 {
        repo_snapshot_id: "sha256:abc".to_string(),
        phase_id: "merge-gate".to_string(),
        config_digest: "sha256:cfg".to_string(),
        tool_identity: "cargo-proof@0.1.0".to_string(),
        proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
    };
    if crate::evaluate_binding_currentness(&expected, None) != crate::BindingCurrentnessV1::Missing
    {
        return Err("missing binding should be missing".to_string());
    }
    let stale = crate::ProofBindingIdentityV1 {
        phase_id: "preflight".to_string(),
        ..expected.clone()
    };
    if crate::evaluate_binding_currentness(&expected, Some(&stale))
        != crate::BindingCurrentnessV1::Stale
    {
        return Err("phase drift should be stale".to_string());
    }
    let incomparable = crate::ProofBindingIdentityV1 {
        tool_identity: "cargo-proof@0.2.0".to_string(),
        ..expected.clone()
    };
    if crate::evaluate_binding_currentness(&expected, Some(&incomparable))
        != crate::BindingCurrentnessV1::Incomparable
    {
        return Err("tool mismatch should be incomparable".to_string());
    }
    Ok(())
}

#[test]
fn provider_envelopes_remain_namespaced_and_distinct() -> Result<(), String> {
    let opaque = crate::ProviderEnvelopeV1 {
        provider_id: "proof-adapter-ripr".to_string(),
        envelope_namespace: "ripr::grip_receipt.v1".to_string(),
        result_class: "opaque".to_string(),
        payload_digest: "sha256:v1:opaque".to_string(),
    };
    let unsupported = crate::ProviderEnvelopeV1 {
        provider_id: "proof-adapter-ripr".to_string(),
        envelope_namespace: "ripr::grip_receipt.v1".to_string(),
        result_class: "unsupported".to_string(),
        payload_digest: "sha256:v1:unsupported".to_string(),
    };
    crate::validate_provider_envelope(&opaque)?;
    crate::validate_provider_envelope(&unsupported)?;
    if !crate::provider_envelope_distinct(&opaque, &unsupported) {
        return Err("opaque and unsupported envelopes must remain distinct".to_string());
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
