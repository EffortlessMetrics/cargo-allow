//! Retained end-to-end evidence for the staged pre-commit gate (#2363).
//!
//! These fixtures deliberately use a small Git repository. They prove that
//! the command consumes staged bytes and emits a bounded, current report, but
//! they do not claim that a local pre-commit run is merge proof.

use allow_diff::{StagedPathRead, read_staged_path, staged_repository_snapshot};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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
            "cargo-allow-precommit-{label}-{}-{nonce}",
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

fn report_value(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn run_precommit(repo: &FixtureRepo, output: &Path, receipt: &Path) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args([
            "check",
            "--root",
            repo.root
                .to_str()
                .ok_or_else(|| "non-UTF-8 fixture root".to_string())?,
            "--profile",
            "spec-system",
            "--phase",
            "precommit",
            "--staged",
            "--format",
            "json",
            "--output",
            output
                .to_str()
                .ok_or_else(|| "non-UTF-8 output path".to_string())?,
            "--receipt",
            receipt
                .to_str()
                .ok_or_else(|| "non-UTF-8 receipt path".to_string())?,
        ])
        .output()
        .map_err(|error| error.to_string())
}

#[test]
fn spec_precommit_conformance() -> Result<(), String> {
    let repo = FixtureRepo::new("candidate")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;

    repo.write("candidate.txt", "staged bytes\n")?;
    repo.git(&["add", "--", "candidate.txt"])?;
    repo.write("candidate.txt", "unstaged worktree bytes\n")?;
    repo.write("unstaged.txt", "never staged\n")?;

    let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
    if read_staged_path(&snapshot, "candidate.txt").map_err(|error| error.to_string())?
        != StagedPathRead::Regular(b"staged bytes\n".to_vec())
    {
        return Err("snapshot did not read staged bytes".to_string());
    }
    if snapshot
        .changes
        .iter()
        .any(|change| change.path.as_deref() == Some(Path::new("unstaged.txt")))
    {
        return Err("unstaged path leaked into the candidate".to_string());
    }

    let output = repo.root.join("target/precommit.json");
    let receipt = repo.root.join("target/precommit.receipt.json");
    fs::create_dir_all(
        output
            .parent()
            .ok_or_else(|| "missing output parent".to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let command = run_precommit(&repo, &output, &receipt)?;
    if command.status.success() {
        return Err("unmapped staged surface unexpectedly passed".to_string());
    }
    let report = report_value(&output)?;
    let receipt_value = report_value(&receipt)?;
    if report != receipt_value {
        return Err("JSON output and receipt diverged".to_string());
    }
    if report.get("result_class").and_then(Value::as_str) != Some("FindingsBlocking") {
        return Err("unmapped staged surface did not return FindingsBlocking".to_string());
    }
    if report.get("staged_identity_before") != report.get("staged_identity_after") {
        return Err("candidate identity changed during a stable evaluation".to_string());
    }
    if report
        .get("findings")
        .and_then(Value::as_array)
        .is_none_or(|findings| {
            !findings.iter().any(|finding| {
                finding.get("code").and_then(Value::as_str)
                    == Some("precommit_unknown_staged_surface")
            })
        })
    {
        return Err("missing unknown staged-surface finding".to_string());
    }
    Ok(())
}

#[test]
fn spec_precommit_no_project_execution() -> Result<(), String> {
    let repo = FixtureRepo::new("no-project-execution")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write(
        "tools/not-run.sh",
        "echo executed > project-executed.marker\n",
    )?;
    repo.git(&["add", "--", "tools/not-run.sh"])?;

    let output = repo.root.join("target/precommit.json");
    let receipt = repo.root.join("target/precommit.receipt.json");
    fs::create_dir_all(
        output
            .parent()
            .ok_or_else(|| "missing output parent".to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let command = run_precommit(&repo, &output, &receipt)?;
    if command.status.success() {
        return Err("unmapped project script unexpectedly passed".to_string());
    }
    if repo.root.join("project-executed.marker").exists() {
        return Err("staged project script was executed".to_string());
    }
    let report = report_value(&output)?;
    if report
        .get("claim_boundary")
        .and_then(Value::as_str)
        .is_none_or(|boundary| !boundary.contains("no project execution"))
    {
        return Err("report omitted the no-project-execution claim boundary".to_string());
    }
    Ok(())
}
