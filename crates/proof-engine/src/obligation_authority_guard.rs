//! Reintroduction guard for the deleted proof-owned obligation model
//! (#3317 / #2936 slice 7).
//!
//! #3314 deleted ChangeObligationPlanV1, the proof-owned obligation loader,
//! planner, and legacy conversion. intent-protocol is the sole obligation
//! input authority. These guards fail if any part of the duplicate
//! obligation authority is reintroduced in the proof family.

#![cfg(test)]

use std::path::{Path, PathBuf};

/// Identifiers that must not reappear in proof-family source after the
/// #3314 deletion.
const FORBIDDEN_IDENTIFIERS: &[&str] = &["ChangeObligationPlan", "proof.change-obligation-plan"];

/// Files deleted by #3314 that must not be recreated under proof-engine.
const FORBIDDEN_PROOF_ENGINE_FILES: &[&str] =
    &["obligation_plan.rs", "planner.rs", "legacy_conversion.rs"];

/// Fixtures deleted by #3314 that must not be recreated.
const FORBIDDEN_FIXTURES: &[&str] = &["obligation-plan-smoke-v1.toml"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn proof_family_does_not_reintroduce_owned_obligation_model() -> Result<(), String> {
    let root = workspace_root();
    // This guard names the forbidden identifiers itself; skip it by its
    // workspace-relative path (file!() is workspace-relative with forward
    // slashes).
    let self_relative = file!().replace('\\', "/");
    let mut sources = Vec::new();
    for family in [
        "crates/proof-engine/src",
        "crates/proof-protocol/src",
        "crates/cargo-proof/src",
    ] {
        collect_rust_sources(&root.join(family), &mut sources);
    }
    if sources.is_empty() {
        return Err("expected proof-family sources to scan".into());
    }
    for path in &sources {
        let relative = path
            .strip_prefix(&root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"));
        if relative.as_deref() == Some(self_relative.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        for identifier in FORBIDDEN_IDENTIFIERS {
            if text.contains(identifier) {
                return Err(format!(
                    "{} reintroduces the deleted proof-owned obligation model ({}); \
                     intent-protocol is the sole obligation input authority (#3314/#3317)",
                    path.display(),
                    identifier
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn deleted_proof_engine_modules_stay_deleted() -> Result<(), String> {
    let root = workspace_root();
    let mut sources = Vec::new();
    collect_rust_sources(&root.join("crates/proof-engine/src"), &mut sources);
    for path in &sources {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if FORBIDDEN_PROOF_ENGINE_FILES.contains(&file_name) {
            return Err(format!(
                "{} was deleted in #3314 and must not be reintroduced",
                path.display()
            ));
        }
    }
    for fixture in FORBIDDEN_FIXTURES {
        let path = root.join("tests/fixtures/cargo-proof").join(fixture);
        if path.is_file() {
            return Err(format!(
                "{} was deleted in #3314 and must not be reintroduced",
                path.display()
            ));
        }
    }
    Ok(())
}

#[test]
fn intent_protocol_is_the_sole_obligation_input_dependency() -> Result<(), String> {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest_text =
        std::fs::read_to_string(&manifest_path).map_err(|err| format!("read manifest: {err}"))?;
    let manifest: toml::Table =
        toml::from_str(&manifest_text).map_err(|err| format!("parse manifest: {err}"))?;
    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "proof-orchestrator manifest is missing [dependencies]".to_string())?;

    if !dependencies.contains_key("intent-protocol") {
        return Err(
            "proof-engine must depend on intent-protocol, the sole obligation input authority"
                .into(),
        );
    }
    // intent-engine and intent-model must stay out of the proof family; their
    // cargo package names are intent-compiler and intent-model.
    for forbidden in ["intent-compiler", "intent-model"] {
        if dependencies.contains_key(forbidden) {
            return Err(format!(
                "proof-engine must not depend on {forbidden}; obligation input is intent-protocol only"
            ));
        }
    }
    Ok(())
}
