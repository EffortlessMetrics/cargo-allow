//! Regression coverage for the source-tree containment guard on `check`
//! `--output`/`--receipt` paths (#1791).
//!
//! The guard must (a) accept paths nested under an out-of-tree `--root` whose
//! parent directories do not yet exist, and (b) reject `..` traversal that
//! would write outside the resolved source-tree root. The first case is what
//! broke when the guard compared against the process cwd instead of the
//! resolved root and used `canonicalize` (which fails on missing parents).
//!
//! This is a focused test: per the repo convention for focused tests
//! (version_output, policy_discovery) it inlines its own subprocess helpers and
//! does not pull in the shared `tests/support` module.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}

fn write_minimal_policy(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), "policy = \"cargo-allow\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

/// A receipt nested under an out-of-tree `--root`, with a not-yet-existing
/// `target/cargo-allow/` parent, must be accepted and written (#1791 regression).
#[test]
fn receipt_under_out_of_tree_root_is_accepted() {
    let root = temp_root("containment-receipt-under-root");
    write_minimal_policy(&root);
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));

    // Audit mode is informational -> check succeeds (exit 0) and writes the
    // receipt even though its parent dir did not pre-exist.
    assert!(
        result.status.success(),
        "check should succeed; stderr=`{}`",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        receipt_output.exists(),
        "receipt should be written under the out-of-tree root"
    );

    remove_temp_root(root);
}

/// An `--output` path that escapes the root via `..` must be rejected before
/// any file is written (#1791 containment contract).
#[test]
fn output_escaping_root_via_parent_traversal_is_rejected() {
    let root = temp_root("containment-output-traversal");
    write_minimal_policy(&root);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let escaping_name = format!("escape-{unique}.md");
    // `../{name}` joined to the absolute root normalizes to a sibling of the
    // root — outside the source-tree root.
    let escaping = root.join(format!("../{escaping_name}"));
    let landing = root
        .parent()
        .map(|p| p.join(&escaping_name))
        .unwrap_or_else(|| root.join(&escaping_name));

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--output")
        .arg(&escaping)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));

    assert!(
        !result.status.success(),
        "check should fail for an escaping --output path; stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("outside the source-tree root"),
        "stderr should explain the containment rejection: `{stderr}`"
    );
    assert!(
        !landing.exists(),
        "no file should be written outside the root at {}",
        landing.display()
    );

    remove_temp_root(root);
}

/// A `--receipt` path that escapes the root via `..` must be rejected; the
/// error receipt itself must not be written at the escaping location.
#[test]
fn receipt_escaping_root_via_parent_traversal_is_rejected() {
    let root = temp_root("containment-receipt-traversal");
    write_minimal_policy(&root);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let escaping_name = format!("escaped-{unique}.receipt.json");
    // `../{name}` (joined to an absolute root) normalizes to a sibling of the
    // root — one level above the source-tree root, i.e. outside the root.
    let escaping_arg = root.join(format!("../{escaping_name}"));
    let landing = root
        .parent()
        .map(|p| p.join(&escaping_name))
        .unwrap_or_else(|| root.join(&escaping_name));

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--receipt")
        .arg(&escaping_arg)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));

    assert!(
        !result.status.success(),
        "check should fail for an escaping --receipt path; stderr=`{}`",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !landing.exists(),
        "no receipt (not even an error receipt) should be written outside the root at {}",
        landing.display()
    );

    remove_temp_root(root);
}
