use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn git_tree_revision_parser_skips_symlinks_and_preserves_newlines() {
    let output = b"100644 blob abc123\tsrc/lib.rs\0\
120000 blob def456\tsrc/link.rs\0\
160000 commit 123456\tvendor/submodule\0\
100644 blob fedcba\tfixtures/line\nbreak.rs\0";

    let files = revision_git::parse_git_ls_tree_z(output);

    assert_eq!(
        files,
        vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("fixtures/line\nbreak.rs")
        ]
    );
}

#[test]
fn git_tree_revision_parser_preserves_executable_modes() {
    let output = b"100644 blob abc123\tREADME.md\0\
100755 blob def456\tscripts/package-proof.sh\0\
120000 blob fedcba\tscripts/link.sh\0";

    let files = revision_git::parse_git_ls_tree_file_entries_z(output);

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].mode, "100644");
    assert_eq!(files[0].path, PathBuf::from("README.md"));
    assert_eq!(files[1].mode, "100755");
    assert_eq!(files[1].path, PathBuf::from("scripts/package-proof.sh"));
}

#[test]
fn git_tree_revision_parser_filters_malformed_records() {
    let output = b"record without separator\0\
\tpath-without-mode\0\
040000 tree abc123\tsrc\0\
100644 blob abc123\tvalid.txt\0\
100644 blob def456\tinvalid-\xff.txt\0";

    let files = revision_git::parse_git_ls_tree_file_entries_z(output);

    assert_eq!(
        files,
        vec![
            revision_git::GitTreeFile {
                mode: "100644".to_string(),
                path: PathBuf::from("valid.txt"),
            },
            revision_git::GitTreeFile {
                mode: "100644".to_string(),
                path: PathBuf::from("invalid-\u{fffd}.txt"),
            },
        ]
    );
}

#[test]
fn parse_git_ls_tree_record_call_presence_observer() {
    assert_eq!(
        revision_git::parse_git_ls_tree_record_for_test(b"record without separator"),
        None
    );
    assert_eq!(
        revision_git::parse_git_ls_tree_record_for_test(b"\tpath-without-mode"),
        None
    );
    assert_eq!(
        revision_git::parse_git_ls_tree_record_for_test(b"040000 tree abc123\tsrc"),
        None
    );

    let entry =
        revision_git::parse_git_ls_tree_record_for_test(b"100644 blob abc123\tinvalid-\xff.txt")
            .unwrap_or_else(|| std::panic::panic_any("file record should parse"));
    assert_eq!(entry.mode, "100644");
    let lossy_path = entry.path.to_string_lossy();
    assert!(lossy_path.starts_with("invalid-"));
    assert!(lossy_path.contains('\u{fffd}'));
    assert!(lossy_path.ends_with(".txt"));
}

#[test]
fn parse_git_ls_tree_record_return_value_discriminator() {
    let entry =
        revision_git::parse_git_ls_tree_record_for_test(b"100755 blob def456\tscripts/run.sh")
            .unwrap_or_else(|| std::panic::panic_any("executable file record should parse"));

    assert_eq!(entry.mode, "100755");
    assert_eq!(entry.path, PathBuf::from("scripts/run.sh"));
}

#[test]
fn revision_git_commands_report_changed_tracked_and_missing_files() {
    let repo = TempGitRepo::new("revision-git-commands");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("README.md", "initial readme\n");
    repo.write("src/lib.rs", "pub fn version() -> u8 { 1 }\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "initial"]);
    let base = repo.git_stdout(&["rev-parse", "HEAD"]);

    repo.write("README.md", "updated readme\n");
    repo.write("src/new.rs", "pub const NEW: bool = true;\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "update"]);

    let changed = revision_git::changed_files(repo.path(), &base, Some("HEAD"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("changed files read: {err}")));
    assert_eq!(
        changed,
        vec![PathBuf::from("README.md"), PathBuf::from("src/new.rs")]
    );

    let tracked = revision_git::git_tracked_files_at_revision(repo.path(), &base)
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked files read: {err}")));
    assert_eq!(
        tracked,
        vec![PathBuf::from("README.md"), PathBuf::from("src/lib.rs")]
    );

    let readme = revision_git::read_file_at_revision(repo.path(), &base, "README.md")
        .unwrap_or_else(|err| std::panic::panic_any(format!("readme read: {err}")));
    assert_eq!(readme.as_deref(), Some("initial readme\n"));

    let lib = revision_git::read_file_at_revision(repo.path(), &base, PathBuf::from("src\\lib.rs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("lib read: {err}")));
    assert_eq!(lib.as_deref(), Some("pub fn version() -> u8 { 1 }\n"));

    let missing = revision_git::read_file_at_revision(repo.path(), &base, "src/new.rs")
        .unwrap_or_else(|err| std::panic::panic_any(format!("missing file handled: {err}")));
    assert_eq!(missing, None);
}

#[test]
fn revision_git_commands_report_git_failures() {
    let repo = TempGitRepo::new("revision-git-failures");
    repo.git(&["init"]);

    let changed_err = revision_git::changed_files(repo.path(), "missing-revision", None)
        .err()
        .unwrap_or_else(|| std::panic::panic_any("missing diff base should fail"));
    assert!(
        changed_err
            .to_string()
            .contains("git diff --name-only failed")
    );

    let tree_err = revision_git::git_tree_files_at_revision(repo.path(), "missing-revision")
        .err()
        .unwrap_or_else(|| std::panic::panic_any("missing tree revision should fail"));
    assert!(
        tree_err
            .to_string()
            .contains("git ls-tree failed for missing-revision")
    );

    let read_err =
        revision_git::read_file_at_revision(repo.path(), "missing-revision", "README.md")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("missing show revision should fail"));
    assert!(
        read_err
            .to_string()
            .contains("failed to read README.md from missing-revision")
    );
}

struct TempGitRepo {
    path: PathBuf,
}

impl TempGitRepo {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("system clock before epoch: {err}"))
            })
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp repo created: {err}")));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| {
                std::panic::panic_any(format!("test parent directory created: {err}"))
            });
        }
        fs::write(&path, contents)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test file written: {err}")));
    }

    fn git(&self, args: &[&str]) {
        let output = self.git_output(args);
        if !output.status.success() {
            std::panic::panic_any(format!(
                "git {args:?} failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let output = self.git_output(args);
        if !output.status.success() {
            std::panic::panic_any(format!(
                "git {args:?} failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn git_output(&self, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git process starts: {err}")))
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
