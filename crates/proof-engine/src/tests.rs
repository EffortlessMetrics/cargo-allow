use std::path::PathBuf;

use crate::provider_api::FakeProofProviderV1;
use proof_protocol::{
    ProofPhaseGatePostureV1, ProofPhaseGateV1, ProofReceiptBindingV1, ProofReceiptSetV1,
};

use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES,
    REQUIRED_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::captured_receipts::CapturedReceiptStoreV1;
use crate::contradiction::detect_contradictions;
use crate::currentness::{evaluate_currentness, receipt_set_digest};
use crate::dry_run::dry_run_proof_plan;
use crate::execution::{ExecutionApprovalV1, evaluate_execution_gate, require_explicit_execution};
use crate::parity::{
    load_ripr_routing_contract, parity_contract_paths, ripr_routing_contract_path,
};
use crate::phase_gate::evaluate_phase_gate;
use crate::provider_registry::{ProviderRegistryV1, register_validated_provider};
use crate::ripr_routing::{
    ProofClaimPostureV1, RiprPreflightClaimInputV1, RiprRouteClaimInputV1, RiprRoutingError,
    compose_preflight_receipt, compose_route_receipt, compose_routing_aggregate,
};
use intent_protocol::{
    IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
    IntentObligationPostureV1, IntentPhaseObligationKindV1, IntentPhaseObligationV1,
    RepositorySnapshotV1, ResolvedRevisionV1,
};
use proof_protocol::ProofResultStateV1;

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-engine/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_engine_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_engine_module".to_string());
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
fn intent_engine_does_not_depend_on_proof_engine() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-engine") {
        return Err(
            "intent-engine must not depend on proof-engine (ADR-0002 forbidden edge)".to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_engine() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-engine") {
        return Err("cargo-allow must not depend on proof-engine".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-engine/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let allowed = fixture
        .get("allowed_upstream_crates")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "parity fixture missing allowed_upstream_crates".to_string())?;
    let allowed: Vec<&str> = allowed.iter().filter_map(|value| value.as_str()).collect();
    for crate_name in ALLOWED_UPSTREAM_CRATES {
        if !allowed.contains(crate_name) {
            return Err(format!(
                "fixture missing allowed upstream crate {crate_name}"
            ));
        }
    }
    for edge in FORBIDDEN_DEPENDENCY_EDGES {
        let Some((from, to)) = edge.split_once(" -> ") else {
            return Err(format!("invalid forbidden edge {edge}"));
        };
        let fixture_edges = fixture
            .get("forbidden_dependency_edges")
            .and_then(|value| value.as_array())
            .ok_or_else(|| "parity fixture missing forbidden_dependency_edges".to_string())?;
        let present = fixture_edges
            .iter()
            .any(|value| value.as_str() == Some(*edge));
        if !present {
            return Err(format!("fixture missing forbidden edge {from} -> {to}"));
        }
    }
    for edge in REQUIRED_DEPENDENCY_EDGES {
        let fixture_edges = fixture
            .get("required_dependency_edges")
            .and_then(|value| value.as_array())
            .ok_or_else(|| "parity fixture missing required_dependency_edges".to_string())?;
        let present = fixture_edges
            .iter()
            .any(|value| value.as_str() == Some(*edge));
        if !present {
            return Err(format!(
                "fixture missing required obligation-input edge {edge} (#3317)"
            ));
        }
    }
    let _ = upstream_surface_markers();
    Ok(())
}

#[test]
fn planner_dry_run_and_execution_gate_pipeline() -> Result<(), String> {
    let identity = IntentIdentityEnvelopeV1::new(
        RepositorySnapshotV1::new_committed_head(
            "identity",
            "sha1",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "abc".to_string(),
                tree: String::new(),
            },
        ),
        IntentArtifactKindV1::RequirementDocument,
        "plan-2589-smoke",
        "test/source.md",
        "test-content",
    );
    let envelope = IntentObligationPlanEnvelopeV1::new(
        identity,
        "precommit",
        vec![IntentPhaseObligationV1 {
            handoff: None,
            obligation_id: "obligation-1".to_string(),
            phase: "precommit".to_string(),
            kind: IntentPhaseObligationKindV1::EvidenceReview,
            statement: "Run cargo-allow no-new".to_string(),
            posture: IntentObligationPostureV1::Blocking,
            evidence_refs: vec![],
        }],
    );
    let mut provider_registry = ProviderRegistryV1::new(Vec::new());
    register_validated_provider(
        &mut provider_registry,
        &FakeProofProviderV1::with_id("cargo-allow"),
    )
    .map_err(|err| err.as_str())?;
    let plan =
        crate::intent_planner::plan_proof_execution_from_intent(&envelope, &provider_registry)
            .map_err(|err| err.as_str())?;
    let dry_run = dry_run_proof_plan(&plan).map_err(|err| err.as_str())?;
    let first_line = dry_run
        .lines
        .first()
        .ok_or_else(|| "dry-run produced no lines".to_string())?;
    if !first_line.structured_argv.starts_with("[structured argv]") {
        return Err("dry-run must emit structured argv only".to_string());
    }
    let denied =
        evaluate_execution_gate(&plan, ExecutionApprovalV1::Denied).map_err(|err| err.as_str())?;
    if denied.would_execute {
        return Err("denied approval must not execute".to_string());
    }
    require_explicit_execution(ExecutionApprovalV1::Denied)
        .err()
        .ok_or_else(|| "denied approval should fail require_explicit_execution".to_string())?;
    let approved = evaluate_execution_gate(&plan, ExecutionApprovalV1::Explicit)
        .map_err(|err| err.as_str())?;
    if !approved.would_execute {
        return Err("explicit approval should allow execution gate".to_string());
    }
    Ok(())
}

#[test]
fn currentness_contradiction_and_phase_gate_evaluate_receipts() -> Result<(), String> {
    let plan_id = "plan-2589-receipts";
    let binding = ProofReceiptBindingV1 {
        binding_id: "binding-1".to_string(),
        plan_id: plan_id.to_string(),
        command_index: 0,
        analysis_receipt_schema_id: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
            .to_string(),
        receipt_digest: "digest-a".to_string(),
    };
    let set = ProofReceiptSetV1::new(plan_id, vec![binding]);
    let digest = receipt_set_digest(&set);
    let mut store = CapturedReceiptStoreV1::new();
    store.capture(set).map_err(|err| err.as_str())?;
    let current =
        evaluate_currentness(&store, plan_id, Some(&digest)).map_err(|err| err.as_str())?;
    if current.status.as_str() != "current" {
        return Err("expected current status".to_string());
    }
    let contradictions =
        detect_contradictions(&store, plan_id, &digest).map_err(|err| err.as_str())?;
    if !contradictions.contradictions.is_empty() {
        return Err("matching digest should not contradict".to_string());
    }
    let gate = ProofPhaseGateV1::new(
        "merge-gate",
        plan_id,
        vec!["binding-1".to_string()],
        ProofPhaseGatePostureV1::Blocking,
    );
    let evaluation = evaluate_phase_gate(&gate, &store).map_err(|err| err.as_str())?;
    if evaluation.outcome.as_str() != "open" {
        return Err("expected open phase gate".to_string());
    }
    Ok(())
}

#[test]
fn parity_fixture_paths_exist() -> Result<(), String> {
    let root = workspace_root();
    for path in parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    Ok(())
}

#[test]
fn ripr_routing_contract_requires_both_phases() -> Result<(), String> {
    let root = workspace_root();
    let contract = load_ripr_routing_contract(&ripr_routing_contract_path(&root))?;
    if !contract.required_phases.contains(&"route".to_string()) {
        return Err("contract must require route phase".to_string());
    }
    if !contract.required_phases.contains(&"preflight".to_string()) {
        return Err("contract must require preflight phase".to_string());
    }
    Ok(())
}

#[test]
fn ripr_routing_non_execution_cannot_compose_to_passed() -> Result<(), String> {
    let snapshot = "sha256:repo-head";
    let plan_id = "plan-2713-non-exec";
    let route = compose_route_receipt(
        snapshot,
        plan_id,
        &[RiprRouteClaimInputV1 {
            claim_id: "claim-route-1".to_string(),
            proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
            posture: ProofClaimPostureV1::Required,
            selected: true,
            provider_registered: true,
            execution_approval: crate::execution::ExecutionApprovalV1::Denied,
            provider_executed: false,
            provider_passed: false,
        }],
    )
    .map_err(routing_err)?;
    let preflight = compose_preflight_receipt(
        snapshot,
        plan_id,
        &[RiprPreflightClaimInputV1 {
            claim_id: "claim-preflight-1".to_string(),
            proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
            posture: ProofClaimPostureV1::Required,
            currentness: proof_protocol::BindingCurrentnessV1::Current,
            gate_outcome: crate::phase_gate::PhaseGateOutcomeV1::Open,
        }],
    )
    .map_err(routing_err)?;
    let aggregate =
        compose_routing_aggregate(snapshot, plan_id, &route, &preflight).map_err(routing_err)?;
    if aggregate.required_aggregate == ProofResultStateV1::ProofPassed {
        return Err("non-execution route must not compose to proof_passed".to_string());
    }
    if aggregate.required_aggregate != ProofResultStateV1::SelectedNotRun {
        return Err(format!(
            "expected selected_not_run aggregate, got {}",
            aggregate.required_aggregate.as_str()
        ));
    }
    Ok(())
}

#[test]
fn ripr_routing_missing_required_claim_blocks_aggregate() -> Result<(), String> {
    let snapshot = "sha256:repo-head";
    let plan_id = "plan-2713-missing";
    let route = compose_route_receipt(
        snapshot,
        plan_id,
        &[RiprRouteClaimInputV1 {
            claim_id: "claim-route-1".to_string(),
            proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
            posture: ProofClaimPostureV1::Required,
            selected: true,
            provider_registered: true,
            execution_approval: crate::execution::ExecutionApprovalV1::Explicit,
            provider_executed: true,
            provider_passed: true,
        }],
    )
    .map_err(routing_err)?;
    let preflight = compose_preflight_receipt(
        snapshot,
        plan_id,
        &[RiprPreflightClaimInputV1 {
            claim_id: "claim-preflight-1".to_string(),
            proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
            posture: ProofClaimPostureV1::Required,
            currentness: proof_protocol::BindingCurrentnessV1::Missing,
            gate_outcome: crate::phase_gate::PhaseGateOutcomeV1::Blocked,
        }],
    )
    .map_err(routing_err)?;
    let aggregate =
        compose_routing_aggregate(snapshot, plan_id, &route, &preflight).map_err(routing_err)?;
    if aggregate.required_aggregate == ProofResultStateV1::ProofPassed {
        return Err("missing required claim must not compose to proof_passed".to_string());
    }
    if aggregate.required_aggregate != ProofResultStateV1::Missing {
        return Err(format!(
            "expected missing aggregate, got {}",
            aggregate.required_aggregate.as_str()
        ));
    }
    Ok(())
}

#[test]
fn ripr_routing_advisory_gap_does_not_block_required_aggregate() -> Result<(), String> {
    let snapshot = "sha256:repo-head";
    let plan_id = "plan-2713-advisory";
    let route = compose_route_receipt(
        snapshot,
        plan_id,
        &[
            RiprRouteClaimInputV1 {
                claim_id: "claim-route-required".to_string(),
                proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
                posture: ProofClaimPostureV1::Required,
                selected: true,
                provider_registered: true,
                execution_approval: crate::execution::ExecutionApprovalV1::Explicit,
                provider_executed: true,
                provider_passed: true,
            },
            RiprRouteClaimInputV1 {
                claim_id: "claim-route-advisory".to_string(),
                proof_reference_id: "crates/ripr/src/lib.rs::tests::advisory_hint".to_string(),
                posture: ProofClaimPostureV1::Advisory,
                selected: true,
                provider_registered: false,
                execution_approval: crate::execution::ExecutionApprovalV1::Denied,
                provider_executed: false,
                provider_passed: false,
            },
        ],
    )
    .map_err(routing_err)?;
    let preflight = compose_preflight_receipt(
        snapshot,
        plan_id,
        &[RiprPreflightClaimInputV1 {
            claim_id: "claim-preflight-required".to_string(),
            proof_reference_id: "crates/ripr/src/lib.rs::tests::proof_smoke".to_string(),
            posture: ProofClaimPostureV1::Required,
            currentness: proof_protocol::BindingCurrentnessV1::Current,
            gate_outcome: crate::phase_gate::PhaseGateOutcomeV1::Open,
        }],
    )
    .map_err(routing_err)?;
    let aggregate =
        compose_routing_aggregate(snapshot, plan_id, &route, &preflight).map_err(routing_err)?;
    if aggregate.required_aggregate != ProofResultStateV1::ProofPassed {
        return Err(format!(
            "advisory gaps must not block required aggregate, got {}",
            aggregate.required_aggregate.as_str()
        ));
    }
    Ok(())
}

fn manifest_lists_dependency(manifest_text: &str, crate_name: &str) -> bool {
    let Ok(table) = toml::from_str::<toml::Table>(manifest_text) else {
        return false;
    };
    table
        .get("dependencies")
        .and_then(|value| value.as_table())
        .is_some_and(|deps| deps.contains_key(crate_name))
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

fn routing_err(err: RiprRoutingError) -> String {
    err.as_str().to_string()
}
