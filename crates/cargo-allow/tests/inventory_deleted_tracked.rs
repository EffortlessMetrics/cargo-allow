//! #2048: a deleted-but-git-tracked file must be disclosed as an inventory
//! diagnostic, not silently dropped. doctor must surface "N tracked file(s)
//! absent from the worktree" so a scan never looks complete while a tracked
//! path disappeared from coverage.
//! #1849: a fresh git repo with no tracked paths must also be disclosed so an
//! empty git-tracked inventory does not look like a complete scan.
//!
//! Focused test: self-contained subprocess helpers + a minimal git fixture.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_allow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-inv-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

fn drop_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove temp root {}: {err}", root.display()),
    }
}

/// Initialize a tiny git repo, commit a file, then delete it from the worktree
/// (so it remains git-tracked but is absent on disk).
fn repo_with_deleted_tracked_file(root: &Path) {
    let git = |args: &[&str]| -> std::process::Output {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap_or_else(|err| panic!("git {:?}: {err}", args))
    };
    fs::write(root.join("kept.txt"), "kept\n")
        .unwrap_or_else(|err| panic!("write kept.txt: {err}"));
    fs::write(root.join("deleted.txt"), "gone\n")
        .unwrap_or_else(|err| panic!("write deleted.txt: {err}"));
    let _ = git(&["init"]);
    // git needs an identity for the commit.
    let _ = git(&["config", "user.email", "test@example.com"]);
    let _ = git(&["config", "user.name", "Test"]);
    let _ = git(&["add", "kept.txt", "deleted.txt"]);
    let _ = git(&["commit", "-m", "seed"]);
    fs::remove_file(root.join("deleted.txt"))
        .unwrap_or_else(|err| panic!("delete tracked file: {err}"));
}

fn repo_with_empty_tracked_set(root: &Path) {
    fs::write(root.join("untracked.txt"), "untracked\n")
        .unwrap_or_else(|err| panic!("write untracked.txt: {err}"));
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("init")
        .output()
        .unwrap_or_else(|err| panic!("git init: {err}"));
    assert!(
        output.status.success(),
        "git init should succeed: stderr=`{}`",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// doctor must report the deleted-tracked file as an inventory warning (human
/// output) and as a JSON diagnostic, not silently scan a complete-looking set.
#[test]
fn doctor_discloses_deleted_tracked_files() {
    let root = temp_root("deleted-tracked");
    repo_with_deleted_tracked_file(&root);

    let human = cargo_allow()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| panic!("run doctor: {err}"));
    assert!(
        human.status.success(),
        "doctor should still exit 0 (advisory): stderr=`{}`",
        String::from_utf8_lossy(&human.stderr)
    );
    let human_text =
        String::from_utf8_lossy(&human.stdout) + String::from_utf8_lossy(&human.stderr);
    assert!(
        human_text.contains("tracked file(s) absent from the worktree"),
        "doctor should disclose deleted-tracked files in human output: `{human_text}`"
    );

    let json = cargo_allow()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| panic!("run doctor json: {err}"));
    let json_text = String::from_utf8_lossy(&json.stdout) + String::from_utf8_lossy(&json.stderr);
    assert!(
        json_text.contains("\"deleted_tracked_files\": 1"),
        "doctor JSON should report deleted_tracked_files=1: `{json_text}`"
    );

    drop_root(root);
}

#[test]
fn doctor_discloses_empty_git_tracked_inventory() {
    let root = temp_root("empty-git-tracked");
    repo_with_empty_tracked_set(&root);

    let human = cargo_allow()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| panic!("run doctor: {err}"));
    assert!(
        human.status.success(),
        "doctor should still exit 0 (advisory): stderr=`{}`",
        String::from_utf8_lossy(&human.stderr)
    );
    let human_text =
        String::from_utf8_lossy(&human.stdout) + String::from_utf8_lossy(&human.stderr);
    assert!(
        human_text.contains("git reported no tracked files"),
        "doctor should disclose empty git-tracked inventory in human output: `{human_text}`"
    );

    let json = cargo_allow()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| panic!("run doctor json: {err}"));
    let json_text = String::from_utf8_lossy(&json.stdout) + String::from_utf8_lossy(&json.stderr);
    assert!(
        json_text.contains("\"empty_git_tracked\": true"),
        "doctor JSON should report empty_git_tracked=true: `{json_text}`"
    );

    drop_root(root);
}
