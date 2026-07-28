//! End-to-end evidence for `cargo intent change status --staged --phase precommit` (#2599-B).

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
            "cargo-intent-change-status-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let repo = Self { root };
        repo.git(&["init", "-q"])?;
        repo.git(&["config", "user.name", "Cargo Intent"])?;
        repo.git(&["config", "user.email", "cargo-intent@example.invalid"])?;
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

fn report_value(stdout: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(stdout).map_err(|error| error.to_string())
}

fn run_change_status(repo: &FixtureRepo) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-intent"))
        .args([
            "--root",
            repo.root
                .to_str()
                .ok_or_else(|| "non-UTF-8 fixture root".to_string())?,
            "--format",
            "json",
            "change",
            "status",
            "--staged",
            "--phase",
            "precommit",
        ])
        .output()
        .map_err(|error| error.to_string())
}

#[test]
fn change_status_unmapped_staged_surface_blocks() -> Result<(), String> {
    let repo = FixtureRepo::new("candidate")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write("candidate.txt", "staged bytes\n")?;
    repo.git(&["add", "--", "candidate.txt"])?;

    let output = run_change_status(&repo)?;
    if output.status.success() {
        return Err("unmapped staged surface unexpectedly passed".to_string());
    }
    let report = report_value(&output.stdout)?;
    if report.get("schema_id").and_then(Value::as_str) != Some("cargo-intent.change-status.v1") {
        return Err("missing change-status schema id".to_string());
    }
    if report.get("result_class").and_then(Value::as_str) != Some("findings") {
        return Err("unmapped staged surface did not return findings".to_string());
    }
    if report
        .get("unmapped_staged_surface")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("report did not mark unmapped staged surface".to_string());
    }
    if report
        .get("claim_boundary")
        .and_then(Value::as_str)
        .is_none_or(|boundary| !boundary.contains("no graph compilation"))
    {
        return Err("report omitted read-only claim boundary".to_string());
    }
    Ok(())
}

#[test]
fn change_status_no_project_execution() -> Result<(), String> {
    let repo = FixtureRepo::new("no-project-execution")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write(
        "tools/not-run.sh",
        "echo executed > project-executed.marker\n",
    )?;
    repo.git(&["add", "--", "tools/not-run.sh"])?;

    let output = run_change_status(&repo)?;
    if output.status.success() {
        return Err("unmapped project script unexpectedly passed".to_string());
    }
    if repo.root.join("project-executed.marker").exists() {
        return Err("staged project script was executed".to_string());
    }
    Ok(())
}

fn run_change_status_analysis_receipt(repo: &FixtureRepo) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-intent"))
        .args([
            "--root",
            repo.root
                .to_str()
                .ok_or_else(|| "non-UTF-8 fixture root".to_string())?,
            "--format",
            "json",
            "change",
            "status",
            "--staged",
            "--phase",
            "precommit",
            "--analysis-receipt",
        ])
        .output()
        .map_err(|error| error.to_string())
}

#[test]
fn change_status_analysis_receipt_envelope() -> Result<(), String> {
    let repo = FixtureRepo::new("analysis-receipt")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write("candidate.txt", "staged bytes\n")?;
    repo.git(&["add", "--", "candidate.txt"])?;

    let output = run_change_status_analysis_receipt(&repo)?;
    let envelope = report_value(&output.stdout)?;
    if envelope.get("schema_id").and_then(Value::as_str) != Some("repo.analysis-receipt.v1") {
        return Err("missing analysis receipt schema id".to_string());
    }
    if envelope.get("provider").and_then(Value::as_str) != Some("cargo-intent") {
        return Err("analysis receipt provider must be cargo-intent".to_string());
    }
    if envelope
        .get("provider_payload_schema")
        .and_then(Value::as_str)
        != Some("cargo-intent.change-status.v1")
    {
        return Err("analysis receipt payload schema mismatch".to_string());
    }
    let payload = envelope
        .get("provider_payload")
        .ok_or_else(|| "missing provider_payload".to_string())?;
    if payload.get("schema_id").and_then(Value::as_str) != Some("cargo-intent.change-status.v1") {
        return Err("provider payload missing change-status schema".to_string());
    }
    Ok(())
}

#[test]
fn change_status_analysis_receipt_rejects_non_json_format() -> Result<(), String> {
    let repo = FixtureRepo::new("analysis-receipt-human")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-intent"))
        .args([
            "--root",
            repo.root
                .to_str()
                .ok_or_else(|| "non-UTF-8 fixture root".to_string())?,
            "--format",
            "human",
            "change",
            "status",
            "--staged",
            "--phase",
            "precommit",
            "--analysis-receipt",
        ])
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Err("human analysis receipt request unexpectedly succeeded".to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("--analysis-receipt requires --format json") {
        return Err(format!("unexpected validation error: {stderr}"));
    }
    Ok(())
}
