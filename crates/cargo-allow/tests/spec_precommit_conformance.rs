//! Retained end-to-end evidence for the staged pre-commit gate (#2363 / #2568).
//!
//! Embedded evaluator removed; precommit requires cargo-intent delegation.

use allow_diff::{StagedPathRead, read_staged_path, staged_repository_snapshot};
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

fn run_precommit(repo: &FixtureRepo) -> Result<Output, String> {
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
            repo.root
                .join("target/precommit.json")
                .to_str()
                .ok_or_else(|| "non-UTF-8 output path".to_string())?,
        ])
        .output()
        .map_err(|error| error.to_string())
}

#[test]
fn staged_snapshot_reads_index_not_worktree() -> Result<(), String> {
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
    Ok(())
}

#[test]
fn spec_precommit_requires_delegation() -> Result<(), String> {
    let repo = FixtureRepo::new("requires-delegation")?;
    repo.write("README.md", "base\n")?;
    repo.git(&["add", "--all"])?;
    repo.git(&["commit", "-qm", "base"])?;
    repo.write("candidate.txt", "staged bytes\n")?;
    repo.git(&["add", "--", "candidate.txt"])?;

    let command = run_precommit(&repo)?;
    if command.status.success() {
        return Err("precommit without delegation should fail".to_string());
    }
    let stderr = String::from_utf8_lossy(&command.stderr);
    if !stderr.contains("delegate_staged_precommit") {
        return Err(format!("unexpected stderr: {stderr}"));
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

    let command = run_precommit(&repo)?;
    if command.status.success() {
        return Err("precommit without delegation should fail".to_string());
    }
    if repo.root.join("project-executed.marker").exists() {
        return Err("staged project script was executed".to_string());
    }
    Ok(())
}
