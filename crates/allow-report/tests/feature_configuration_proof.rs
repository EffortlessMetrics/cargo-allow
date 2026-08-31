//! Proves the checked-in feature-configuration matrix rows (#3905) by
//! running every selected configuration through real cargo invocations and
//! by asserting closure laws against `cargo tree` and the compiled test
//! surface.
//!
//! The matrix in `allow-report` is the single source of configuration IDs
//! and selected features; this proof consumes it through the typed API so
//! the workflow and the matrix can never drift apart. PR B proved the
//! allow-rust and allow-files rows; this slice proves the cargo-proof
//! default, per-provider, and all-providers rows. cargo-intent and the
//! shared substrate packages define no `[features]`, so the issue's
//! do-not-manufacture law leaves them without rows.

use allow_report::NoDefaultFeaturesPostureV1;
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
/// selects the cargo subcommand and target breadth, the posture adds
/// `--no-default-features`, and explicit features are passed through
/// `--features`. Deeper proof depths (package, installed, interop) belong
/// to their own journeys and cannot be synthesized here.
fn cargo_args_for(
    row: &allow_report::SupportedFeatureConfigurationV1,
) -> Result<Vec<String>, String> {
    let mut args = match row.proof_depth {
        allow_report::FeatureConfigurationProofDepthV1::CompileOnly => {
            vec!["check".to_string()]
        }
        allow_report::FeatureConfigurationProofDepthV1::UnitAndDocTests => {
            vec!["test".to_string()]
        }
        allow_report::FeatureConfigurationProofDepthV1::AllTargets => {
            vec!["test".to_string(), "--all-targets".to_string()]
        }
        other => {
            return Err(format!(
                "proof depth {} is owned by packaging and installed journeys, \
                 not this harness",
                other.as_str()
            ));
        }
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
    Ok(args)
}

/// Rows at or below AllTargets are provable by this harness; deeper rows
/// wait for the packaging and installed journeys that own their depths.
fn harness_provable(row: &allow_report::SupportedFeatureConfigurationV1) -> bool {
    matches!(
        row.proof_depth,
        allow_report::FeatureConfigurationProofDepthV1::CompileOnly
            | allow_report::FeatureConfigurationProofDepthV1::UnitAndDocTests
            | allow_report::FeatureConfigurationProofDepthV1::AllTargets
    )
}

fn run_proof(config_id: &str) -> Result<(), String> {
    let root = repo_root()?;
    let matrix = supported_feature_configuration_matrix();
    let row = matrix
        .rows
        .iter()
        .find(|row| row.configuration_id == config_id)
        .ok_or_else(|| format!("configuration {config_id} missing from the matrix"))?;
    let args = cargo_args_for(row)?;
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

/// Run `cargo tree` from the repo root with `--locked` always appended and
/// return its stdout for closure-law assertions.
fn cargo_tree_text(extra_args: &[&str]) -> Result<String, String> {
    let root = repo_root()?;
    let mut args = vec!["tree".to_string()];
    args.extend(extra_args.iter().map(|arg| arg.to_string()));
    args.push("--locked".to_string());
    let output = Command::new(env!("CARGO"))
        .args(&args)
        .current_dir(&root)
        .output()
        .map_err(|error| format!("spawn cargo tree: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo tree {extra_args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run `cargo test -- --list` for cargo-proof and return the test-id lines
/// (each ends with `: test`; cargo warnings never do).
fn cargo_proof_test_list(extra_args: &[&str]) -> Result<String, String> {
    let root = repo_root()?;
    let mut args: Vec<String> = ["test", "-p", "cargo-proof", "--locked"]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    args.extend(extra_args.iter().map(|arg| arg.to_string()));
    args.push("--".to_string());
    args.push("--list".to_string());
    let output = Command::new(env!("CARGO"))
        .args(&args)
        .current_dir(&root)
        .output()
        .map_err(|error| format!("spawn cargo test --list: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo test --list {extra_args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let listed: Vec<&str> = stdout
        .lines()
        .filter(|line| line.ends_with(": test"))
        .collect();
    Ok(listed.join("\n"))
}

/// Totality law: every matrix row at or below AllTargets proves green in
/// this harness, and a row can only be proven if the matrix declares it.
/// Rows keep their declared depth: a green shallower run never substitutes
/// for a deeper proof (#3905).
#[test]
fn every_executable_matrix_row_proves_green() -> Result<(), String> {
    let matrix = supported_feature_configuration_matrix();
    if matrix.rows.is_empty() {
        return Err("the feature-configuration matrix must not be empty".to_string());
    }
    for row in &matrix.rows {
        if harness_provable(row) {
            run_proof(&row.configuration_id)?;
        }
    }
    Ok(())
}

#[test]
fn minimal_model_excludes_tree_sitter_from_the_normal_closure() -> Result<(), String> {
    let text = cargo_tree_text(&["-p", "allow-rust", "-e", "normal", "--no-default-features"])?;
    if text.contains("tree-sitter") {
        return Err(
            "minimal-model closure must not contain tree-sitter in any spelling".to_string(),
        );
    }
    Ok(())
}

/// The provider cfg gates must bind real code: default selects no provider
/// test surface, each single feature activates exactly its own provider
/// module tests and never another provider's, and all-providers activates
/// both test-bearing providers. Hawk ships fixtures without unit tests, so
/// its activation is proven by compilation in the row proof above; absence
/// of a provider feature must never read as semantic proof success (#3905
/// negative control 6).
#[test]
fn cargo_proof_provider_gates_bind_the_compiled_test_surface() -> Result<(), String> {
    let gated: [(&str, &str); 3] = [
        ("provider-cargo-allow", "providers::cargo_allow::"),
        ("provider-hawk", "providers::hawk::"),
        ("provider-ripr", "providers::ripr::"),
    ];
    let test_bearing: [&str; 2] = ["providers::cargo_allow::", "providers::ripr::"];

    let default_list = cargo_proof_test_list(&[])?;
    for (_, prefix) in gated {
        if default_list.contains(prefix) {
            return Err(format!(
                "default cargo-proof test surface must not include {prefix}"
            ));
        }
    }
    for (feature, prefix) in gated {
        let list = cargo_proof_test_list(&["--features", feature])?;
        if test_bearing.contains(&prefix) && !list.contains(prefix) {
            return Err(format!("feature {feature} must activate {prefix} tests"));
        }
        for (_, other) in gated {
            if other != prefix && list.contains(other) {
                return Err(format!(
                    "feature {feature} must not activate foreign prefix {other}"
                ));
            }
        }
    }
    let all_list = cargo_proof_test_list(&["--features", "all-providers"])?;
    for prefix in test_bearing {
        if !all_list.contains(prefix) {
            return Err(format!(
                "the all-providers feature closure must activate {prefix} tests"
            ));
        }
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

/// Every deliberately unselected feature combination must stay unscheduled:
/// no selected row of the same package may carry that feature set in any
/// order (#3905 matrix law).
#[test]
fn non_selected_combinations_are_never_scheduled_rows() -> Result<(), String> {
    let matrix = supported_feature_configuration_matrix();
    for non_selection in &matrix.explicit_non_selections {
        let mut combo = non_selection.selected_features.clone();
        combo.sort();
        for row in &matrix.rows {
            if row.root_package_name != non_selection.package_name {
                continue;
            }
            let mut selected = row.explicit_features.clone();
            selected.sort();
            if selected == combo {
                return Err(format!(
                    "non-selected combination {:?} on {} must not appear as row {}",
                    non_selection.selected_features,
                    non_selection.package_name,
                    row.configuration_id
                ));
            }
        }
    }
    Ok(())
}
