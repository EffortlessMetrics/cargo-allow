//! Characterization and cutover evidence for spec-system delegation (#2601-C).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE: &str =
    include_str!("../../../tests/compat/fixtures/intent-spec-system-cutover-v1.toml");
const RECEIPT: &str =
    include_str!("../../../tests/compat/fixtures/cargo-allow-spec-system-cutover-receipt-v1.toml");

#[test]
fn intent_spec_system_cutover_fixture_pins_contract() {
    for needle in [
        "cargo-allow.intent-spec-system-cutover.v1",
        "cargo-allow.intent-delegation.v1",
        "delegate_spec_system",
        "delegate_staged_precommit",
        "precommit evaluator",
        "forbidden_when_cutover_enabled",
        "cargo-intent",
    ] {
        assert!(FIXTURE.contains(needle), "fixture missing {needle}");
    }
}

#[test]
fn cargo_allow_spec_system_cutover_receipt_pins_reachability() {
    for needle in [
        "CargoAllowCompatibilityCutover",
        "embedded spec-system CI audit retired",
        "#2568",
    ] {
        assert!(RECEIPT.contains(needle), "receipt missing {needle}");
    }
}

struct FixtureRepo {
    root: PathBuf,
}

impl FixtureRepo {
    fn new(label: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-spec-system-cutover-{label}-{}-{nonce}",
            std::process::id()
        ));
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

fn write_cutover_config(root: &Path, staged_precommit: bool) -> Result<(), String> {
    let config_dir = root.join(".allow/compatibility");
    fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
    let staged_flag = if staged_precommit { "true" } else { "false" };
    let config = format!(
        r#"schema_id = "cargo-allow.intent-delegation.v1"
delegate_spec_system = true
delegate_staged_precommit = {staged_flag}
timeout_secs = 30
"#
    );
    fs::write(config_dir.join("intent-delegation.toml"), config).map_err(|error| error.to_string())
}

fn run_cargo_allow(args: &[&str]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .output()
        .map_err(|error| error.to_string())
}

fn root_arg(repo: &FixtureRepo) -> Result<String, String> {
    repo.root
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| "non-UTF-8 fixture root".to_string())
}

#[test]
fn spec_system_check_fails_closed_when_cutover_enabled() -> Result<(), String> {
    let repo = FixtureRepo::new("check-fail-closed")?;
    write_cutover_config(&repo.root, false)?;
    let output = run_cargo_allow(&[
        "check",
        "--root",
        &root_arg(&repo)?,
        "--profile",
        "spec-system",
    ])?;
    if output.status.success() {
        return Err("spec-system check should fail closed under cutover".to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("delegate_spec_system") {
        return Err(format!("unexpected stderr: {stderr}"));
    }
    Ok(())
}

#[test]
fn spec_system_precommit_fails_closed_without_staged_delegate() -> Result<(), String> {
    let repo = FixtureRepo::new("precommit-fail-closed")?;
    write_cutover_config(&repo.root, false)?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write("candidate.txt", "staged bytes\n")?;
    repo.git(&["add", "--", "candidate.txt"])?;

    let output = run_cargo_allow(&[
        "check",
        "--root",
        &root_arg(&repo)?,
        "--profile",
        "spec-system",
        "--phase",
        "precommit",
        "--staged",
    ])?;
    if output.status.success() {
        return Err("embedded precommit evaluator should fail closed under cutover".to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("precommit evaluator") {
        return Err(format!("unexpected stderr: {stderr}"));
    }
    Ok(())
}

#[test]
fn spec_system_worklist_fails_closed_when_cutover_enabled() -> Result<(), String> {
    let repo = FixtureRepo::new("worklist-fail-closed")?;
    write_cutover_config(&repo.root, false)?;
    let output = run_cargo_allow(&[
        "worklist",
        "--root",
        &root_arg(&repo)?,
        "--profile",
        "spec-system",
    ])?;
    if output.status.success() {
        return Err("spec-system worklist should fail closed under cutover".to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("worklist") {
        return Err(format!("unexpected stderr: {stderr}"));
    }
    Ok(())
}
