//! Actual retained transport, native readback and packaged-command boundaries.

use effortless_repo_protocol::{CompletenessV1, ResultClassV1};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn core_command_migration_native_transport_and_admission() -> Result<(), String> {
    let root = repository_root()?;
    let proof = OwnedDirectory::new("native")?;
    let observations = proof.0.join("observations");
    let generation = source_generation(&root)?;
    let mut completed = None;
    for executable in ["python3", "python"] {
        let mut command = child(executable);
        command
            .current_dir(&root)
            .arg("-B")
            .arg(root.join("scripts/test-command-case-evidence.py"))
            .arg("--native-bin")
            .arg(env!("CARGO_BIN_EXE_cargo-allow"))
            .arg("--source-generation")
            .arg(&generation)
            .arg("--export-observations")
            .arg(&observations);
        match command.output() {
            Ok(output) => {
                completed = Some(output);
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("start transport/native controls: {error}")),
        }
    }
    let output = completed.ok_or("Python 3 is required for transport/native admission tests")?;
    require(
        output.status.success(),
        format!(
            "transport/native controls failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    // Keep the actual inner test results in --nocapture validation logs.
    println!("{}", String::from_utf8_lossy(&output.stderr));
    let schema: Value = serde_json::from_slice(
        &fs::read(root.join("docs/schemas/cargo-allow.command-case-evidence.v1.schema.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let validator = jsonschema::validator_for(&schema).map_err(|error| error.to_string())?;
    let classes = serde_json::to_value([
        ResultClassV1::Completed,
        ResultClassV1::Findings,
        ResultClassV1::NotProven,
        ResultClassV1::PartialData,
        ResultClassV1::StaleInput,
        ResultClassV1::Unsupported,
        ResultClassV1::MalformedInput,
        ResultClassV1::InstrumentFailure,
        ResultClassV1::Cancelled,
        ResultClassV1::Conflict,
    ])
    .map_err(|error| error.to_string())?;
    let completeness = serde_json::to_value([
        CompletenessV1::Complete,
        CompletenessV1::Partial,
        CompletenessV1::Unknown,
    ])
    .map_err(|error| error.to_string())?;
    require(
        schema.pointer("/$defs/case_admission/properties/observed_result_class/anyOf/0/enum")
            == Some(&classes)
            && schema
                .pointer("/$defs/case_admission/properties/observed_completeness/anyOf/0/enum")
                == Some(&completeness),
        "evidence schema vocabulary differs from the native protocol",
    )?;
    for name in ["catalogue", "context", "bundle", "admission"] {
        let value: Value = serde_json::from_slice(
            &fs::read(observations.join(format!("{name}.json")))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let errors = validator
            .iter_errors(&value)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        require(
            errors.is_empty(),
            format!("actual {name} violates the evidence schema: {errors:?}"),
        )?;
    }
    Ok(())
}

#[test]
fn core_command_migration_explicit_help_and_package_containment() -> Result<(), String> {
    let top = native(&["--help"])?;
    require(
        top.status.success()
            && !String::from_utf8_lossy(&top.stdout).contains("command-migration-evidence"),
        "internal reader leaked into the normal help surface",
    )?;
    let explicit = native(&["command-migration-evidence", "--help"])?;
    let help = String::from_utf8_lossy(&explicit.stdout);
    require(
        explicit.status.success()
            && help.contains("--catalogue")
            && help.contains("--expected-context")
            && help.contains("--bundle")
            && help.contains("without executing a candidate"),
        format!("hidden reader has no accurate reachable help: {help}"),
    )?;
    let prefixed = native(&["allow", "command-migration-evidence", "--help"])?;
    require(
        prefixed.status.success() && prefixed.stdout == explicit.stdout,
        "cargo-prefixed reader help differs from the direct command",
    )?;
    let direct_missing = native(&["command-migration-evidence"])?;
    let prefixed_missing = native(&["allow", "command-migration-evidence"])?;
    require(
        direct_missing.status.code() == Some(2)
            && prefixed_missing.status == direct_missing.status
            && prefixed_missing.stderr == direct_missing.stderr
            && String::from_utf8_lossy(&direct_missing.stderr).contains("--catalogue"),
        "cargo-prefixed reader did not reach its ordinary required-input validation",
    )?;
    for arguments in [&["allow"][..], &["allow", "unknown-command"][..]] {
        let refused = native(arguments)?;
        require(
            refused.status.code() == Some(2)
                && String::from_utf8_lossy(&refused.stderr).contains("unrecognized subcommand 'allow'"),
            "unknown/bare allow token was incorrectly stripped as a Cargo shim",
        )?;
    }
    let root = repository_root()?;
    let model = fs::read_to_string(root.join("crates/cargo-allow/src/core_command_migration.rs"))
        .map_err(|error| error.to_string())?;
    let catalogue = fs::read(root.join("docs/release/core-command-migration-cases.v1.json"))
        .map_err(|error| error.to_string())?;
    let digest = allow_core::sha256_v1_bytes(&catalogue);
    require(
        model.contains(&format!("\"{digest}\""))
            && !model.contains("include_bytes!")
            && !model.contains("include_str!"),
        "native catalogue pin drifted or depends on checkout-only files at package build time",
    )?;
    let catalogue: Value = serde_json::from_slice(&catalogue).map_err(|error| error.to_string())?;
    let dimensions = catalogue
        .get("dimensions")
        .and_then(Value::as_array)
        .ok_or("missing dimensions")?;
    let cases = catalogue
        .get("cases")
        .and_then(Value::as_array)
        .ok_or("missing cases")?;
    require(
        dimensions.len() == 18
            && cases.len() == 107
            && cases
                .iter()
                .filter(|case| case.get("first_family_collector") == Some(&Value::Bool(true)))
                .count()
                == 29,
        "accepted denominator or bounded A selection changed without a contract update",
    )?;
    for negative in [
        "internal_leakage",
        "extraction_leakage",
        "sibling_requirement",
        "spec_requirement",
        "experimental_promotion",
        "maturity_disagreement",
        "hidden_reachability",
        "alias_compatibility",
        "write_posture",
        "forbidden_proof",
        "package_help_mismatch",
        "published_channel_snippets",
        "first_screen_crowding",
    ] {
        let id = Value::String(format!("E.help.{negative}"));
        require(
            cases.iter().any(|case| {
                case.get("id") == Some(&id)
                    && case.get("first_family_collector") == Some(&Value::Bool(false))
            }),
            format!("required unresolved #3882 negative control is absent: {negative}"),
        )?;
    }
    Ok(())
}

fn native(arguments: &[&str]) -> Result<Output, String> {
    child(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| error.to_string())
}

fn source_generation(root: &Path) -> Result<String, String> {
    let output = child("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| error.to_string())?;
    require(
        output.status.success(),
        "could not observe the tested source generation",
    )?;
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| error.to_string())
}

fn child(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for key in ["PATH", "SystemRoot", "WINDIR", "TMP", "TEMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
}

struct OwnedDirectory(PathBuf);

impl OwnedDirectory {
    fn new(label: &str) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temporary = std::env::temp_dir()
            .canonicalize()
            .map_err(|error| error.to_string())?;
        for _ in 0..128 {
            let path = temporary.join(format!(
                "cargo-allow-command-migration-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("could not exclusively reserve a command-migration test directory".to_string())
    }
}

impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
