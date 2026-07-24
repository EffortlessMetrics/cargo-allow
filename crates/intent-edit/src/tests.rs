use std::path::PathBuf;

use crate::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, EVALUATOR_PACKET_MODULE_ID,
    FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use crate::edit_plan::{
    IntentEditActionKindV1, IntentEditActionV1, IntentEditPlanError, IntentEditPlanV1,
    IntentEditTargetResolutionV1, stable_action_id, validate_edit_plan,
};
use crate::edit_plan_surface::EditPlanSurface;
use crate::parity::{
    edit_plan_parity_contract_paths, load_edit_plan_parity_contract, parity_contract_paths,
};

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
