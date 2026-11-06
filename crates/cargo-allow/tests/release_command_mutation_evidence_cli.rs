//! Retained #4725 C-family mutation transport controls and committed evidence
//! replay.

use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn command_mutation_manifest_replay_and_transport_controls() -> Result<(), String> {
    // #4461: under coverage instrumentation the sanitized child environment of
    // the transport harness cannot run the instrumented binary; the same
    // documented skip applies to this transport-driven suite.
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return Ok(());
    }
    let root = repository_root()?;
    let catalogue = root.join("docs/release/core-command-migration-cases.v1.json");
    let manifest_path = root
        .join("docs/release/evidence")
        .join("command-mutation-1")
        .join("manifest.v1.json");
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
        format!(
            "committed mutation-evidence manifest is unreadable at {}: {error}",
            manifest_path.display()
        )
    })?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("committed manifest is not valid JSON: {error}"))?;
    let binary = manifest
        .get("binary")
        .ok_or("committed manifest lacks a binary identity")?;
    let provenance = binary
        .get("provenance")
        .and_then(Value::as_str)
        .ok_or("committed manifest lacks a binary provenance")?;
    require(
        manifest.get("schema_id").and_then(Value::as_str)
            == Some("cargo-allow.command-mutation-manifest.v1"),
        "committed manifest does not pin the mutation-manifest schema",
    )?;
    require(
        provenance == "supplied_installed_candidate",
        "committed manifest must retain an installed-candidate provenance",
    )?;
    require(
        binary
            .get("candidate_identity")
            .is_some_and(|value| !value.is_null())
            && binary
                .get("install_identity")
                .is_some_and(|value| !value.is_null()),
        "committed manifest lacks the exact candidate/install identities",
    )?;
    let cases = manifest
        .get("cases")
        .and_then(Value::as_array)
        .ok_or("committed manifest lacks a case array")?;
    let obligations = [
        "C.init.preview",
        "C.init.alternate_config",
        "C.init.apply",
        "C.init.stale_conflict",
        "C.init.presentation",
        "C.propose.preview",
        "C.propose.candidate",
        "C.propose.replacement_boundary",
        "C.propose.stale_conflict",
        "C.propose.presentation",
        "C.add.preview",
        "C.add.candidate",
        "C.add.apply",
        "C.add.stale_conflict",
        "C.add.presentation",
        "C.add_from_plan.unsupported_preview",
        "C.add_from_plan.plan_linkage",
        "C.add_from_plan.apply",
        "C.add_from_plan.stale_conflict",
        "C.add_from_plan.presentation",
    ];
    for obligation in obligations {
        require(
            cases
                .iter()
                .any(|case| case.get("case_id").and_then(Value::as_str) == Some(obligation)),
            format!("committed manifest omits retained obligation {obligation}"),
        )?;
    }
    // Replay the committed manifest through the production admission reader.
    let completed = run_python(
        &root,
        [
            "-B",
            "scripts/verify-command-mutation-evidence.py",
            "verify-manifest",
            "--catalogue",
            catalogue.to_string_lossy().as_ref(),
            "--manifest",
            manifest_path.to_string_lossy().as_ref(),
        ],
    )?;
    require(
        completed.status.success(),
        format!(
            "committed manifest failed replay admission:\n{}\n{}",
            String::from_utf8_lossy(&completed.stdout),
            String::from_utf8_lossy(&completed.stderr)
        ),
    )?;
    // Hermetic transport/admission controls (stub collections; no scanner run).
    let mut arguments = vec![
        "-B".to_string(),
        "scripts/test-command-mutation-evidence.py".to_string(),
        "--native-bin".to_string(),
        env!("CARGO_BIN_EXE_cargo-allow").to_string(),
    ];
    if std::env::var("CARGO_ALLOW_MUTATION_SKIP_TRANSPORT").as_deref() == Ok("1") {
        arguments.clear();
    }
    if !arguments.is_empty() {
        let completed = run_python(&root, &arguments)?;
        require(
            completed.status.success(),
            format!(
                "hermetic mutation transport controls failed:\n{}\n{}",
                String::from_utf8_lossy(&completed.stdout),
                String::from_utf8_lossy(&completed.stderr)
            ),
        )?;
    }
    Ok(())
}

fn run_python(
    root: &Path,
    arguments: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<std::process::Output, String> {
    let arguments: Vec<String> = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_string())
        .collect();
    let mut completed = None;
    let mut last_error = None;
    for executable in ["python3", "python"] {
        let mut command = child(executable);
        command.current_dir(root);
        for argument in &arguments {
            command.arg(argument);
        }
        match command.output() {
            Ok(output) => {
                completed = Some(output);
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => last_error = Some(error),
            Err(error) => return Err(format!("start transport controls: {error}")),
        }
    }
    completed.ok_or_else(|| {
        format!(
            "Python 3 is required for mutation transport controls ({:?})",
            last_error
        )
    })
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| error.to_string())
}

fn child(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for key in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "TMP",
        "TEMP",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
