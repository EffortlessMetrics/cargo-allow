//! Characterization and end-to-end evidence for staged precommit delegation (#2601-B).

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE: &str =
    include_str!("../../../tests/fixtures/cargo-allow/intent-staged-precommit-delegate-v1.toml");

#[test]
fn intent_staged_precommit_delegate_fixture_pins_contract() {
    for needle in [
        "cargo-allow.intent-staged-precommit-delegate.v1",
        "cargo-allow.intent-delegation.v1",
        "repo.analysis-receipt.v1",
        "cargo-intent.change-status.v1",
        "delegate_staged_precommit",
        "identity_mismatch",
        "--analysis-receipt",
    ] {
        assert!(FIXTURE.contains(needle), "fixture missing {needle}");
    }
}

struct FixtureRepo {
    root: PathBuf,
}

impl FixtureRepo {
    fn new(label: &str) -> Result<Self, String> {
        let nonce = fixture_nonce()?;
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-intent-delegate-{label}-{}-{nonce}",
            std::process::id()
        ));
        Self::at(root)
    }

    fn at(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let repo = Self { root };
        repo.git(&["init", "-q"])?;
        repo.git(&["config", "user.name", "Cargo Allow"])?;
        repo.git(&["config", "user.email", "cargo-allow@example.invalid"])?;
        Ok(repo)
    }

    fn write(&self, relative: &str, contents: &str) -> Result<(), String> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, contents).map_err(|error| error.to_string())
    }

    fn git(&self, args: &[&str]) -> Result<Output, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(format!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}

impl Drop for FixtureRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture_nonce() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())
        .map(|duration| duration.as_nanos())
}

fn cargo_intent_binary() -> Result<PathBuf, String> {
    if let Ok(bin) = std::env::var("CARGO_BIN_EXE_cargo-intent") {
        return Ok(PathBuf::from(bin));
    }
    let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(cargo_intent_bin_name());
    if bin.is_file() {
        return Ok(bin);
    }
    Err(format!(
        "cargo-intent binary missing at {}; dev-dependency build should produce it",
        bin.display()
    ))
}

fn cargo_intent_bin_name() -> &'static str {
    if cfg!(windows) {
        "cargo-intent.exe"
    } else {
        "cargo-intent"
    }
}

fn write_delegation_config(root: &Path, executable: &Path) -> Result<(), String> {
    let config_dir = root.join(".allow/compatibility");
    fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
    let executable_text = executable
        .to_str()
        .ok_or_else(|| "provider executable path is not UTF-8".to_string())?;
    let config = format!(
        r#"schema_id = "cargo-allow.intent-delegation.v1"
executable = {executable_text:?}
delegate_staged_precommit = true
timeout_secs = 30
"#
    );
    fs::write(config_dir.join("intent-delegation.toml"), config).map_err(|error| error.to_string())
}

fn run_delegated_precommit(repo: &FixtureRepo, output: &Path) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--root")
        .arg(&repo.root)
        .arg("--profile")
        .arg("spec-system")
        .arg("--phase")
        .arg("precommit")
        .arg("--staged")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(output)
        .output()
        .map_err(|error| error.to_string())
}

fn assert_delegated_report(output_path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(output_path).map_err(|error| error.to_string())?;
    let report: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let gates = report
        .get("remaining_gates")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing remaining_gates".to_string())?;
    if !gates
        .iter()
        .any(|gate| gate.as_str() == Some("delegated via repo.analysis-receipt.v1"))
    {
        return Err("report did not record analysis-receipt delegation".to_string());
    }
    Ok(bytes)
}

fn prepare_staged_repo(repo: &FixtureRepo) -> Result<(), String> {
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write("candidate.txt", "staged bytes\n")?;
    repo.git(&["add", "--", "candidate.txt"])?;
    Ok(())
}

#[test]
fn cargo_intent_rejects_analysis_receipt_without_json() -> Result<(), String> {
    let provider = cargo_intent_binary()?;
    let repo = FixtureRepo::new("receipt-format")?;
    let output = Command::new(provider)
        .arg("--root")
        .arg(&repo.root)
        .arg("--format")
        .arg("human")
        .arg("change")
        .arg("status")
        .arg("--staged")
        .arg("--phase")
        .arg("precommit")
        .arg("--analysis-receipt")
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Err("non-JSON analysis receipt unexpectedly succeeded".to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("--analysis-receipt requires --format json") {
        return Err(format!("unexpected usage diagnostic: {stderr}"));
    }
    Ok(())
}

#[test]
fn delegated_staged_precommit_uses_analysis_receipt() -> Result<(), String> {
    let provider = cargo_intent_binary()?;
    let repo = FixtureRepo::new("delegated")?;
    write_delegation_config(&repo.root, &provider)?;
    prepare_staged_repo(&repo)?;

    let output_path = repo.root.join("target/precommit.json");
    let command = run_delegated_precommit(&repo, &output_path)?;
    if command.status.success() {
        return Err("delegated unmapped staged surface unexpectedly passed".to_string());
    }
    assert_delegated_report(&output_path)?;
    Ok(())
}

#[test]
fn delegated_staged_precommit_drains_large_analysis_receipt() -> Result<(), String> {
    let provider = cargo_intent_binary()?;
    let repo = FixtureRepo::new("large-delegated")?;
    write_delegation_config(&repo.root, &provider)?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;

    for index in 0..1_400 {
        repo.write(&format!("bulk/candidate-{index:04}.txt"), "staged bytes\n")?;
    }
    repo.git(&["add", "--all"])?;

    let output_path = repo.root.join("target/large-precommit.json");
    let command = run_delegated_precommit(&repo, &output_path)?;
    if command.status.success() {
        return Err("large delegated unmapped staged surface unexpectedly passed".to_string());
    }
    let stderr = String::from_utf8_lossy(&command.stderr);
    if stderr.contains("Timeout") || stderr.contains("provider exceeded") {
        return Err(format!("large delegated receipt timed out: {stderr}"));
    }
    let report = assert_delegated_report(&output_path)?;
    if report.len() <= 64 * 1024 {
        return Err(format!(
            "large delegation fixture did not exceed an ordinary pipe buffer: {} bytes",
            report.len()
        ));
    }
    Ok(())
}
