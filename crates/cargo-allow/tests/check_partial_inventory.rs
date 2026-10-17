use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn unique_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// #4594: the main check path must fail no-new on a partial inventory the
/// same way the staged path does — a worktree-deleted tracked file makes
/// the scan incomplete, and a scan that silently missed tracked files must
/// not read as a green pass (the staged path already fails this exact
/// condition).
#[test]
fn check_no_new_fails_on_partial_inventory_from_deleted_tracked_file() {
    let root = unique_root("check-partial-inventory");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "self-receipt for the fixture policy"
evidence = ["doc:policy/allow.toml"]
created = "2026-05-09"
review_after = "2027-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture"]);

    // Baseline: complete inventory, gate passes.
    let baseline = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--mode")
        .arg("no-new")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run baseline check: {err}")));
    assert!(
        baseline.status.success(),
        "a complete receipted fixture must pass: {}",
        String::from_utf8_lossy(&baseline.stderr)
    );

    // Delete a tracked source file: the git_tracked inventory can no longer
    // see it, so completeness drops to partial.
    fs::remove_file(root.join("src/lib.rs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("delete source: {err}")));

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--mode")
        .arg("no-new")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "a partial-inventory no-new run must fail the gate: {stdout}"
    );
    assert!(
        stdout.contains("partial"),
        "the run must disclose the partial inventory: {stdout}"
    );
}
