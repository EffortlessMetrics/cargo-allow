//! Proves the checked-in feature-configuration matrix rows (#3905 PR B) by
//! running each selected configuration through real cargo invocations and by
//! asserting the minimal-model closure law against `cargo tree`.
//!
//! The matrix in `allow-report` is the single source of configuration IDs
//! and selected features; this proof consumes it through the typed API so
//! the workflow and the matrix can never drift apart. cargo-proof provider
//! rows are a later slice (issue PR C scope).

use allow_report::NoDefaultFeaturesPostureV1;
use allow_report::SupportedFeatureConfigurationV1;
use allow_report::supported_feature_configuration_matrix;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|crates| crates.parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| "no repo root above the manifest dir".to_string())
}

/// Derive the exact cargo arguments for one matrix row: the proof depth
/// selects check vs test, the posture adds `--no-default-features`, and
/// explicit features are passed through `--features`.
fn cargo_args_for(row: &SupportedFeatureConfigurationV1) -> Vec<String> {
    let mut args = match row.proof_depth {
        allow_report::FeatureConfigurationProofDepthV1::CompileOnly => {
            vec!["check".to_string()]
        }
        _ => vec!["test".to_string()],
    };
    args.push("-p".to_string());
    args.push(row.root_package_name.clone());
    args.push("--locked".to_string());
    if row.no_default_features == NoDefaultFeaturesPostureV1::NoDefaultFeatures {
        args.push("--no-default-features".to_string());
    }
    if !row.explicit_features.is_empty() {
        args.push("--features".to_string());
        args.push(row.explicit_features.join(","));
    }
    args
}

fn run_proof(config_id: &str) -> Result<(), String> {
    let root = repo_root()?;
    let matrix = supported_feature_configuration_matrix();
    let row = matrix
        .rows
        .iter()
        .find(|row| row.configuration_id == config_id)
        .ok_or_else(|| format!("configuration {config_id} missing from the matrix"))?;
    let args = cargo_args_for(row);
    let output = Command::new(env!("CARGO"))
        .args(&args)
        .current_dir(&root)
        .output()
        .map_err(|error| format!("spawn cargo {config_id}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "proof for {config_id} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[test]
fn allow_rust_rows_prove_green() -> Result<(), String> {
    for config_id in [
        "allow-rust.default",
        "allow-rust.minimal-model",
        "allow-rust.syntax-explicit",
    ] {
        run_proof(config_id)?;
    }
    Ok(())
}

#[test]
fn allow_files_rows_prove_green() -> Result<(), String> {
    for config_id in ["allow-files.default", "allow-files.changie"] {
        run_proof(config_id)?;
    }
    Ok(())
}

#[test]
fn minimal_model_excludes_tree_sitter_from_the_normal_closure() -> Result<(), String> {
    let root = repo_root()?;
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "allow-rust",
            "-e",
            "normal",
            "--no-default-features",
            "--locked",
        ])
        .current_dir(&root)
        .output()
        .map_err(|error| format!("spawn cargo tree: {error}"))?;
    if !output.status.success() {
        return Err("cargo tree failed for the minimal-model closure".to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.contains("tree-sitter") {
        return Err(
            "minimal-model closure must not contain tree-sitter in any spelling".to_string(),
        );
    }
    Ok(())
}

#[test]
fn unknown_configuration_ids_are_absent_from_the_matrix() -> Result<(), String> {
    let matrix = supported_feature_configuration_matrix();
    let powerset_probe = "allow-rust.powerset";
    if matrix
        .rows
        .iter()
        .any(|row| row.configuration_id == powerset_probe)
    {
        return Err(format!(
            "the matrix must stay a finite selected list, found {powerset_probe}"
        ));
    }
    Ok(())
}
