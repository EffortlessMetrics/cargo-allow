use super::*;
use crate::{OutputFormat, RootArgs};
use allow_core::CargoAllowErrorKind;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn cmd_diff_with_explicit_head_reports_missing_default_policy_config_with_exact_error() {
    let root = diff_fixture_dir();
    init_git_repo_without_policy(&root);

    let err = cmd_diff(&DiffArgs {
        artifact_dir: None,
        emit: None,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Human,
        output: None,
        receipt: None,
        base: Some("HEAD~1".to_string()),
        head: Some("HEAD".to_string()),
        require_change_note: false,
        revisions_dir: std::path::PathBuf::from(".allow/revisions"),
        write_change_note_template: None,
    })
    .expect_err("diff without policy in compared revisions should fail");

    // #2511/#3275: a missing or unresolvable policy config is typed
    // `InvalidConfig`, not the untyped `Unknown` fallback.
    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_diff_with_explicit_head_rejects_missing_explicit_config_path_with_exact_error() {
    let root = diff_fixture_dir();
    init_git_repo_with_policy(&root);

    let err = cmd_diff(&DiffArgs {
        artifact_dir: None,
        emit: None,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(PathBuf::from("missing-policy.toml")),
        kind: None,
        include_untracked: false,
        format: OutputFormat::Human,
        output: None,
        receipt: None,
        base: Some("HEAD~1".to_string()),
        head: Some("HEAD".to_string()),
        require_change_note: false,
        revisions_dir: std::path::PathBuf::from(".allow/revisions"),
        write_change_note_template: None,
    })
    .expect_err("diff with missing explicit --config in compared revisions should fail");

    // #2511/#3275: a missing or unresolvable policy config is typed
    // `InvalidConfig`, not the untyped `Unknown` fallback.
    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_diff_writes_receipt_for_clean_in_process_run() {
    let root = diff_fixture_dir();
    init_git_repo_with_policy(&root);
    let receipt = root.join("target/cargo-allow/diff.receipt.json");

    cmd_diff(&DiffArgs {
        artifact_dir: None,
        emit: None,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(PathBuf::from("policy/allow.toml")),
        kind: None,
        include_untracked: false,
        format: OutputFormat::Human,
        output: None,
        receipt: Some(receipt.clone()),
        base: Some("HEAD".to_string()),
        head: None,
        require_change_note: false,
        revisions_dir: std::path::PathBuf::from(".allow/revisions"),
        write_change_note_template: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("clean diff should succeed: {err}")));

    let receipt_text = fs::read_to_string(&receipt)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read diff receipt: {err}")));
    let receipt_value: serde_json::Value = serde_json::from_str(&receipt_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse diff receipt: {err}")));
    assert_eq!(
        receipt_value
            .get("command")
            .and_then(serde_json::Value::as_str),
        Some("diff"),
        "in-process diff receipt command"
    );
    assert_eq!(
        receipt_value
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("passed"),
        "in-process clean diff receipt status"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

static NEXT_DIFF_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn diff_fixture_dir() -> PathBuf {
    let id = NEXT_DIFF_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-diff-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn init_git_repo_without_policy(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(root.join("src/lib.rs"), "fn base() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write base source: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    fs::write(root.join("src/lib.rs"), "fn head() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write head source: {err}")));
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "head"]);
}

fn init_git_repo_with_policy(root: &Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(root.join("src/lib.rs"), "fn base() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write base source: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

[[allow]]
id = "allow-base"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-11-01"

[allow.selector]
ast_kind = "tracked_file"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    fs::write(root.join("src/lib.rs"), "fn head() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write head source: {err}")));
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "head"]);
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}
