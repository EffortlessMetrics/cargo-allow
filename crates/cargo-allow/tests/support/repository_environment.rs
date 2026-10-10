//! Keep disposable Git fixtures independent of a caller's repository selection.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn isolate_repository(command: &mut Command) -> &mut Command {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    command
}

struct Canary(PathBuf);

impl Drop for Canary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Re-run an actual fixture test with hostile variables on that child only.
/// The index-only case reproduces `git -C` staging into a foreign index; the
/// full case also redirects repository, worktree, and object-store selection.
pub fn require_isolated_fixture_test(test_name: &str) -> Result<(), String> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let canary = Canary(std::env::temp_dir().join(format!(
        "cargo-allow-git-environment-canary-{}-{unique}",
        std::process::id(),
    )));
    fs::create_dir_all(canary.0.join("src")).map_err(|error| error.to_string())?;
    for path in ["src/subject.rs", "src/lib.rs"] {
        fs::write(canary.0.join(path), b"// external index canary\n")
            .map_err(|error| error.to_string())?;
    }
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.name", "cargo-allow canary"],
        vec!["config", "user.email", "canary@example.invalid"],
        vec!["add", "src"],
        vec!["-c", "commit.gpgsign=false", "commit", "-q", "-m", "canary"],
    ] {
        let mut command = Command::new("git");
        command.arg("-C").arg(&canary.0).args(&args);
        let output = isolate_repository(&mut command)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "canary git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr),
            ));
        }
    }
    let git_dir = canary.0.join(".git");
    let index = git_dir.join("index");
    let index_before = fs::read(&index).map_err(|error| error.to_string())?;
    let head_before = fs::read(git_dir.join("HEAD")).map_err(|error| error.to_string())?;
    for index_only in [true, false] {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut command = Command::new(executable);
        isolate_repository(&mut command);
        command
            .args(["--exact", test_name, "--nocapture", "--color", "never"])
            .env("GIT_INDEX_FILE", &index);
        if !index_only {
            command
                .env("GIT_DIR", &git_dir)
                .env("GIT_WORK_TREE", &canary.0)
                .env("GIT_COMMON_DIR", &git_dir)
                .env("GIT_OBJECT_DIRECTORY", git_dir.join("objects"))
                .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", git_dir.join("objects"));
        }
        let output = command.output().map_err(|error| error.to_string())?;
        if fs::read(&index).map_err(|error| error.to_string())? != index_before
            || fs::read(git_dir.join("HEAD")).map_err(|error| error.to_string())? != head_before
        {
            return Err(format!(
                "fixture {test_name} changed the external canary (index_only={index_only})"
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || !stdout.contains(&format!("test {test_name} ... ok")) {
            return Err(format!(
                "isolated fixture {test_name} did not pass (index_only={index_only}): {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr),
            ));
        }
    }
    Ok(())
}
