use std::path::PathBuf;

use crate::approval_currentness::{
    ApprovalCurrentnessError, IntentEditApprovalCurrentnessV1, IntentEditApprovalStateV1,
    validate_approval_currentness,
};
use crate::approval_currentness_surface::ApprovalCurrentnessSurface;
use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, EVALUATOR_PACKET_MODULE_ID,
    FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::dialect_adapter::{CANONICAL_DIALECT_IDS, IntentEditDialectV1, adapt_selector};
use crate::dialect_adapter_surface::DialectAdapterSurface;
use crate::edit_plan::{
    IntentEditActionKindV1, IntentEditActionV1, IntentEditPlanError, IntentEditPlanV1,
    IntentEditTargetResolutionV1, stable_action_id, validate_edit_plan,
};
use crate::edit_plan_surface::EditPlanSurface;
use crate::parity::{
    approval_currentness_parity_contract_paths, dialect_adapter_parity_contract_paths,
    edit_plan_parity_contract_paths, load_approval_currentness_parity_contract,
    load_dialect_adapter_parity_contract, load_edit_plan_parity_contract,
    load_recompile_contract_parity_contract, load_repo_edit_translation_parity_contract,
    load_settlement_parity_contract, parity_contract_paths,
    recompile_contract_parity_contract_paths, repo_edit_translation_parity_contract_paths,
    settlement_parity_contract_paths,
};
use crate::recompile_contract::{
    TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID, compile_recompile_contract, validate_recompile_contract,
};
use crate::recompile_contract_surface::RecompileContractSurface;
use crate::repo_edit_translation::{RepoEditTranslationError, translate_plan_to_repo_edit};
use crate::repo_edit_translation_surface::RepoEditTranslationSurface;
use crate::settlement::{
    IntentEditResidualObligationKindV1, compile_settlement_plan, validate_settlement_plan,
};
use crate::settlement_surface::SettlementSurface;

#[test]
fn intent_engine_surface_matches_topology_marker() -> Result<(), String> {
    if intent_engine::EvaluatorPacketSurface::MODULE_ID != EVALUATOR_PACKET_MODULE_ID {
        return Err("intent-engine surface marker drifted from topology contract".to_string());
    }
    Ok(())
}

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/intent-edit/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("intent_edit_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing intent_edit_module".to_string());
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
fn intent_engine_does_not_depend_on_intent_edit() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "intent-edit") {
        return Err(
            "intent-engine must not depend on intent-edit (ADR-0002 forbidden edge)".to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_intent_edit() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_dependency(&text, "intent-edit") {
        return Err("cargo-allow must not depend on intent-edit (product boundary)".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/intent-edit/parity-boundary-v1.toml");
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
    let markers = upstream_surface_markers();
    if markers.is_empty() {
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
fn edit_plan_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = edit_plan_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing edit plan parity fixture path".to_string())?;
    let contract = load_edit_plan_parity_contract(&contract_path)?;
    if contract.intent_edit_module != EditPlanSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            EditPlanSurface::MODULE_ID,
            contract.intent_edit_module
        ));
    }
    Ok(())
}

#[test]
fn stable_action_id_is_deterministic() -> Result<(), String> {
    let id = stable_action_id(IntentEditActionKindV1::ReplaceFile, "policy/allow.toml")
        .map_err(|err| err.as_str().to_string())?;
    let again = stable_action_id(IntentEditActionKindV1::ReplaceFile, "policy/allow.toml")
        .map_err(|err| err.as_str().to_string())?;
    if id != again {
        return Err("stable action id must be deterministic".to_string());
    }
    if !id.starts_with("intent-edit:replace_file:") {
        return Err(format!("unexpected action id prefix: {id}"));
    }
    Ok(())
}

#[test]
fn validate_edit_plan_requires_find_before_create() -> Result<(), String> {
    let selector = "policy/allow.toml";
    let action_id = stable_action_id(IntentEditActionKindV1::CreateFile, selector)
        .map_err(|err| err.as_str())?;
    let plan = IntentEditPlanV1::new(
        "plan-missing-find",
        vec![IntentEditActionV1 {
            action_id,
            kind: IntentEditActionKindV1::CreateFile,
            resolution: IntentEditTargetResolutionV1::CreateIfMissing {
                selector: selector.to_string(),
                relative_path: selector.to_string(),
            },
        }],
    );
    match validate_edit_plan(&plan) {
        Err(IntentEditPlanError::MissingFindBeforeCreate { .. }) => Ok(()),
        other => Err(format!(
            "expected missing_find_before_create, got {other:?}"
        )),
    }
}

#[test]
fn validate_edit_plan_accepts_find_then_create() -> Result<(), String> {
    let selector = "policy/allow.toml";
    let find_id = stable_action_id(IntentEditActionKindV1::ReplaceFile, selector)
        .map_err(|err| err.as_str())?;
    let create_id = stable_action_id(IntentEditActionKindV1::CreateFile, selector)
        .map_err(|err| err.as_str())?;
    let plan = IntentEditPlanV1::new(
        "plan-find-create",
        vec![
            IntentEditActionV1 {
                action_id: find_id,
                kind: IntentEditActionKindV1::ReplaceFile,
                resolution: IntentEditTargetResolutionV1::FindExisting {
                    selector: selector.to_string(),
                },
            },
            IntentEditActionV1 {
                action_id: create_id,
                kind: IntentEditActionKindV1::CreateFile,
                resolution: IntentEditTargetResolutionV1::CreateIfMissing {
                    selector: selector.to_string(),
                    relative_path: selector.to_string(),
                },
            },
        ],
    );
    validate_edit_plan(&plan).map_err(|err| err.as_str().to_string())
}

#[test]
fn dialect_adapter_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = dialect_adapter_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing dialect adapter parity fixture path".to_string())?;
    let contract = load_dialect_adapter_parity_contract(&contract_path)?;
    if contract.intent_edit_module != DialectAdapterSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            DialectAdapterSurface::MODULE_ID,
            contract.intent_edit_module
        ));
    }
    for dialect_id in CANONICAL_DIALECT_IDS {
        if !contract
            .canonical_dialect_ids
            .iter()
            .any(|entry| entry == dialect_id)
        {
            return Err(format!("fixture missing dialect id {dialect_id}"));
        }
    }
    Ok(())
}

#[test]
fn adapt_selector_normalizes_cargo_allow_paths() -> Result<(), String> {
    let adapted = adapt_selector(
        IntentEditDialectV1::CargoAllowPolicy,
        "./policy\\allow.toml",
    )
    .map_err(|err| err.as_str().to_string())?;
    if adapted != "policy/allow.toml" {
        return Err(format!("unexpected adapted selector: {adapted}"));
    }
    Ok(())
}

#[test]
fn adapt_selector_strips_allow_prefix_for_spec_system() -> Result<(), String> {
    let adapted = adapt_selector(
        IntentEditDialectV1::SpecSystem,
        ".allow/spec-system/evidence/x.toml",
    )
    .map_err(|err| err.as_str().to_string())?;
    if adapted != "spec-system/evidence/x.toml" {
        return Err(format!("unexpected adapted selector: {adapted}"));
    }
    Ok(())
}

#[test]
fn approval_currentness_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = approval_currentness_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing approval currentness parity fixture path".to_string())?;
    let contract = load_approval_currentness_parity_contract(&contract_path)?;
    if contract.intent_edit_module != ApprovalCurrentnessSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            ApprovalCurrentnessSurface::MODULE_ID,
            contract.intent_edit_module
        ));
    }
    Ok(())
}

#[test]
fn validate_approval_currentness_fails_closed_on_stale() -> Result<(), String> {
    let envelope = IntentEditApprovalCurrentnessV1::new(
        "plan-stale",
        IntentEditApprovalStateV1::Approved,
        effortless_repo_protocol::CurrentnessV1::Stale,
        "sha256:v1:deadbeef",
    );
    match validate_approval_currentness(&envelope) {
        Err(ApprovalCurrentnessError::StaleCurrentness) => Ok(()),
        other => Err(format!("expected stale_currentness, got {other:?}")),
    }
}

#[test]
fn validate_approval_currentness_accepts_approved_current() -> Result<(), String> {
    let envelope = IntentEditApprovalCurrentnessV1::new(
        "plan-approved",
        IntentEditApprovalStateV1::Approved,
        effortless_repo_protocol::CurrentnessV1::Current,
        "sha256:v1:deadbeef",
    );
    validate_approval_currentness(&envelope).map_err(|err| err.as_str().to_string())
}

#[test]
fn repo_edit_translation_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = repo_edit_translation_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing repo-edit translation parity fixture path".to_string())?;
    let contract = load_repo_edit_translation_parity_contract(&contract_path)?;
    if contract.intent_edit_module != RepoEditTranslationSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            RepoEditTranslationSurface::MODULE_ID,
            contract.intent_edit_module
        ));
    }
    Ok(())
}

#[test]
fn translate_plan_to_repo_edit_maps_replace_file() -> Result<(), String> {
    let selector = "policy/allow.toml";
    let action_id = stable_action_id(IntentEditActionKindV1::ReplaceFile, selector)
        .map_err(|err| err.as_str())?;
    let plan = IntentEditPlanV1::new(
        "plan-translate",
        vec![IntentEditActionV1 {
            action_id: action_id.clone(),
            kind: IntentEditActionKindV1::ReplaceFile,
            resolution: IntentEditTargetResolutionV1::FindExisting {
                selector: selector.to_string(),
            },
        }],
    );
    let approval = IntentEditApprovalCurrentnessV1::new(
        "plan-translate",
        IntentEditApprovalStateV1::Approved,
        effortless_repo_protocol::CurrentnessV1::Current,
        "sha256:v1:abc",
    );
    let translation =
        translate_plan_to_repo_edit(&plan, &approval, IntentEditDialectV1::CargoAllowPolicy)
            .map_err(|err| err.as_str())?;
    let Some(request) = translation.requests.first() else {
        return Err("expected one translated request".to_string());
    };
    if translation.requests.len() != 1 {
        return Err("expected exactly one translated request".to_string());
    }
    if request.target != "policy/allow.toml" {
        return Err(format!("unexpected target {}", request.target));
    }
    if request.mode != effortless_repo_edit::SingleTargetApplyMode::AtomicReplace {
        return Err("expected atomic replace mode".to_string());
    }
    if request.caller_reference != action_id {
        return Err("caller reference should preserve action id".to_string());
    }
    Ok(())
}

#[test]
fn translate_plan_to_repo_edit_rejects_delete_file() -> Result<(), String> {
    let selector = "policy/allow.toml";
    let action_id = stable_action_id(IntentEditActionKindV1::DeleteFile, selector)
        .map_err(|err| err.as_str())?;
    let plan = IntentEditPlanV1::new(
        "plan-delete",
        vec![IntentEditActionV1 {
            action_id,
            kind: IntentEditActionKindV1::DeleteFile,
            resolution: IntentEditTargetResolutionV1::FindExisting {
                selector: selector.to_string(),
            },
        }],
    );
    let approval = IntentEditApprovalCurrentnessV1::new(
        "plan-delete",
        IntentEditApprovalStateV1::Approved,
        effortless_repo_protocol::CurrentnessV1::Current,
        "sha256:v1:abc",
    );
    match translate_plan_to_repo_edit(&plan, &approval, IntentEditDialectV1::CargoAllowPolicy) {
        Err(RepoEditTranslationError::UnsupportedActionKind { .. }) => Ok(()),
        other => Err(format!("expected unsupported_action_kind, got {other:?}")),
    }
}

#[test]
fn recompile_contract_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = recompile_contract_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing recompile contract parity fixture path".to_string())?;
    let contract = load_recompile_contract_parity_contract(&contract_path)?;
    if contract.intent_edit_module != RecompileContractSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            RecompileContractSurface::MODULE_ID,
            contract.intent_edit_module
        ));
    }
    if contract.target_transport_schema_id != TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID {
        return Err("fixture transport schema drifted from intent-engine".to_string());
    }
    Ok(())
}

#[test]
fn compile_recompile_contract_emits_phase_obligation_plan() -> Result<(), String> {
    let selector = "policy/allow.toml";
    let action_id = stable_action_id(IntentEditActionKindV1::ReplaceFile, selector)
        .map_err(|err| err.as_str())?;
    let plan = IntentEditPlanV1::new(
        "plan-recompile",
        vec![IntentEditActionV1 {
            action_id: action_id.clone(),
            kind: IntentEditActionKindV1::ReplaceFile,
            resolution: IntentEditTargetResolutionV1::FindExisting {
                selector: selector.to_string(),
            },
        }],
    );
    let approval = IntentEditApprovalCurrentnessV1::new(
        "plan-recompile",
        IntentEditApprovalStateV1::Approved,
        effortless_repo_protocol::CurrentnessV1::Current,
        "sha256:v1:abc",
    );
    let translation =
        translate_plan_to_repo_edit(&plan, &approval, IntentEditDialectV1::CargoAllowPolicy)
            .map_err(|err| err.as_str())?;
    let contract = compile_recompile_contract(&translation);
    validate_recompile_contract(&translation, &contract).map_err(|err| err.as_str())?;
    let transport = contract.to_phase_obligation_transport_plan();
    if transport.schema_id != TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID {
        return Err("phase obligation plan schema drifted".to_string());
    }
    if transport.obligations.is_empty() {
        return Err("expected recompile obligations for policy edit".to_string());
    }
    let toml = toml::to_string(&transport).map_err(|err| err.to_string())?;
    let parsed = intent_engine::load_phase_obligation_plan_toml(&toml)
        .map_err(|err| format!("intent-engine transport parse failed: {err}"))?;
    if parsed.obligations.is_empty() {
        return Err("intent-engine rejected recompile transport obligations".to_string());
    }
    Ok(())
}

#[test]
fn settlement_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = settlement_parity_contract_paths(&root)
        .into_iter()
        .next()
        .ok_or_else(|| "missing settlement parity fixture path".to_string())?;
    let contract = load_settlement_parity_contract(&contract_path)?;
    if contract.intent_edit_module != SettlementSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            SettlementSurface::MODULE_ID,
            contract.intent_edit_module
        ));
    }
    for kind in [
        IntentEditResidualObligationKindV1::AwaitApplyReceipt,
        IntentEditResidualObligationKindV1::AwaitRecompileProof,
        IntentEditResidualObligationKindV1::AwaitCurrentnessRefresh,
    ] {
        if !contract
            .required_residual_kinds
            .iter()
            .any(|entry| entry == kind.as_str())
        {
            return Err(format!("fixture missing residual kind {}", kind.as_str()));
        }
    }
    Ok(())
}

#[test]
fn compile_settlement_plan_emits_residual_obligations() -> Result<(), String> {
    let selector = "policy/allow.toml";
    let action_id = stable_action_id(IntentEditActionKindV1::ReplaceFile, selector)
        .map_err(|err| err.as_str())?;
    let plan = IntentEditPlanV1::new(
        "plan-settlement",
        vec![IntentEditActionV1 {
            action_id: action_id.clone(),
            kind: IntentEditActionKindV1::ReplaceFile,
            resolution: IntentEditTargetResolutionV1::FindExisting {
                selector: selector.to_string(),
            },
        }],
    );
    let approval = IntentEditApprovalCurrentnessV1::new(
        "plan-settlement",
        IntentEditApprovalStateV1::Approved,
        effortless_repo_protocol::CurrentnessV1::Current,
        "sha256:v1:abc",
    );
    let settlement =
        compile_settlement_plan(&plan, &approval, IntentEditDialectV1::CargoAllowPolicy)
            .map_err(|err| err.as_str())?;
    validate_settlement_plan(&settlement).map_err(|err| err.as_str())?;
    if settlement.residual_obligations.len() < 3 {
        return Err("expected apply, recompile, and currentness residuals".to_string());
    }
    let has_apply = settlement.residual_obligations.iter().any(|item| {
        item.kind == IntentEditResidualObligationKindV1::AwaitApplyReceipt
            && item.action_id.as_deref() == Some(action_id.as_str())
    });
    if !has_apply {
        return Err("missing await_apply_receipt residual".to_string());
    }
    let has_recompile = settlement
        .residual_obligations
        .iter()
        .any(|item| item.kind == IntentEditResidualObligationKindV1::AwaitRecompileProof);
    if !has_recompile {
        return Err("missing await_recompile_proof residual".to_string());
    }
    let has_currentness = settlement
        .residual_obligations
        .iter()
        .any(|item| item.kind == IntentEditResidualObligationKindV1::AwaitCurrentnessRefresh);
    if !has_currentness {
        return Err("missing await_currentness_refresh residual".to_string());
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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
