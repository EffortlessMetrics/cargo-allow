use std::path::PathBuf;

use proof_protocol::ProofPlanCommandV1;
use proof_protocol::ProofReceiptBindingV1;

use super::boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
use super::command_registry::{default_cargo_allow_registry, validate_command_registry};
use super::command_spec::{CommandSpecError, compile_invocation_spec, reject_prose_as_executable};
use super::dry_run::{DryRunCommandReportV1, render_structured_argv};
use super::parity::parity_contract_paths;
use super::receipt_interpretation::{CommandReceiptStatusV1, interpret_receipt_binding};

#[test]
fn boundary_surface_matches_parity_contract_module() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-adapter-command/parity-boundary-v1.toml");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .map_err(|err| format!("read parity fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse parity fixture: {err}"))?;
    let Some(module) = fixture
        .get("proof_adapter_command_module")
        .and_then(|value| value.as_str())
    else {
        return Err("parity fixture missing proof_adapter_command_module".to_string());
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
fn reject_prose_as_executable_blocks_issue_markdown() -> Result<(), String> {
    let prose = "Run `rtk cargo run -p cargo-allow -- check --mode no-new` before merge.";
    match reject_prose_as_executable(prose) {
        Err(super::command_spec::CommandSpecError::ProseNotExecutable) => Ok(()),
        other => Err(format!("expected prose_not_executable, got {other:?}")),
    }
}

#[test]
fn compile_invocation_spec_binds_registry_argv() -> Result<(), String> {
    let registry = default_cargo_allow_registry();
    validate_command_registry(&registry).map_err(|err| err.as_str())?;
    let plan_command = ProofPlanCommandV1::new(
        "cargo-allow",
        vec![
            "check".to_string(),
            "--mode".to_string(),
            "no-new".to_string(),
        ],
    );
    let spec = compile_invocation_spec(&registry, "cargo-allow.check.no-new", &plan_command)
        .map_err(|err| err.as_str())?;
    if spec.program != "cargo-allow" {
        return Err("unexpected program".to_string());
    }
    let dry_run = DryRunCommandReportV1::from_invocation_spec(&spec);
    let rendered = render_structured_argv(&dry_run);
    if !rendered.starts_with("[structured argv]") {
        return Err("dry-run must not emit pasteable shell".to_string());
    }
    let binding = ProofReceiptBindingV1 {
        binding_id: "cargo-allow.check.no-new:0".to_string(),
        plan_id: "plan-1".to_string(),
        command_index: 0,
        analysis_receipt_schema_id: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
            .to_string(),
        receipt_digest: "sha256:v1:abc".to_string(),
    };
    let outcome = interpret_receipt_binding(&spec, &binding);
    if outcome.status != CommandReceiptStatusV1::Bound {
        return Err("expected bound receipt outcome".to_string());
    }
    Ok(())
}

#[test]
fn registry_preserves_prefix_compatibility_and_exact_report_binding() -> Result<(), String> {
    let registry = default_cargo_allow_registry();
    let existing = ProofPlanCommandV1::new(
        "cargo-allow",
        vec![
            "check".to_string(),
            "--mode".to_string(),
            "no-new".to_string(),
            "--root".to_string(),
            ".".to_string(),
        ],
    );
    compile_invocation_spec(&registry, "cargo-allow.check.no-new", &existing)
        .map_err(|err| err.as_str())?;

    let report = ProofPlanCommandV1::new(
        "cargo-allow",
        vec![
            "capabilities".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ],
    );
    compile_invocation_spec(&registry, "cargo-allow.capabilities.json", &report)
        .map_err(|err| err.as_str())?;

    let mut mutating_report = report;
    mutating_report.args.push("--output".to_string());
    mutating_report.args.push("report.json".to_string());
    match compile_invocation_spec(&registry, "cargo-allow.capabilities.json", &mutating_report) {
        Err(CommandSpecError::ArgvTrailingArgs { command_id })
            if command_id == "cargo-allow.capabilities.json" =>
        {
            Ok(())
        }
        Ok(_) => Err("exact capability report accepted trailing arguments".to_string()),
        Err(error) => Err(format!("unexpected exact argv error: {error:?}")),
    }
}

#[test]
fn intent_engine_does_not_depend_on_proof_adapter_command() -> Result<(), String> {
    let manifest = workspace_root().join("crates/intent-engine/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read intent-engine manifest: {err}"))?;
    if manifest_lists_dependency(&text, "proof-adapter-command") {
        return Err(
            "intent-engine must not depend on proof-adapter-command (ADR-0002 forbidden edge)"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn cargo_allow_does_not_depend_on_proof_adapter_command() -> Result<(), String> {
    let manifest = workspace_root().join("crates/cargo-allow/Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_any_dependency(&text, "proof-adapter-command") {
        return Err("cargo-allow must not depend on proof-adapter-command".to_string());
    }
    Ok(())
}

#[test]
fn allowed_upstream_topology_registered() -> Result<(), String> {
    let root = workspace_root();
    let fixture_path = root.join("tests/fixtures/proof-adapter-command/parity-boundary-v1.toml");
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
