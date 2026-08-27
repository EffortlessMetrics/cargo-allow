use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
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

fn write_foreign_allow_toml(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"
schema_version = "1"
owner = "repo-policy"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write foreign allow.toml: {err}")));
}

fn write_native_ledger(root: &std::path::Path) {
    fs::write(
        root.join("policy/cargo-allow.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write native cargo-allow.toml: {err}")));
}

#[test]
fn list_discovers_native_ledger_beside_foreign_allow_toml() {
    let root = temp_root("policy-discovery-side-by-side");
    write_foreign_allow_toml(&root);
    write_native_ledger(&root);

    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["list", "--format", "json"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run list: {err}")));

    assert_status("list", &output, true);
    remove_temp_root(root);
}

#[test]
fn list_reports_skipped_foreign_allow_toml_when_no_native_ledger_exists() {
    let root = temp_root("policy-discovery-foreign-only");
    write_foreign_allow_toml(&root);

    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["list", "--format", "json"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run list: {err}")));

    assert_status("list", &output, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("skipped 1 foreign-dialect candidate"),
        "expected foreign-dialect skip diagnostic, got: {stderr}"
    );
    assert!(
        stderr.contains("allow.toml"),
        "expected skipped path in diagnostic, got: {stderr}"
    );
    assert!(
        stderr.contains("missing policy = \"cargo-allow\" marker"),
        "expected dialect marker diagnostic, got: {stderr}"
    );
    remove_temp_root(root);
}

#[test]
fn list_honors_explicit_config_over_foreign_default_path() {
    let root = temp_root("policy-discovery-explicit-config");
    write_foreign_allow_toml(&root);
    fs::write(
        root.join("policy/explicit.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write explicit policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "list",
            "--format",
            "json",
            "--config",
            "policy/explicit.toml",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run list: {err}")));

    assert_status("list", &output, true);
    remove_temp_root(root);
}

/// A ledger that exists but cannot be selected is not an unconfigured tree.
///
/// `audit` tolerates a missing policy so first-run adoption works, but the
/// same fallback used to swallow an unusable ledger: the scan ran against an
/// empty config and the report claimed an advisory pass over receipts nobody
/// could read (#1952).
#[test]
fn audit_fails_closed_when_a_policy_candidate_cannot_be_selected() {
    let root = temp_root("policy-discovery-audit-unusable");
    write_foreign_allow_toml(&root);

    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["audit", "--format", "markdown"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));

    assert_status("audit", &output, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("skipped 1 foreign-dialect candidate"),
        "expected the skipped candidate to be named, got: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("passed (advisory)"),
        "audit must not claim an advisory pass over an unusable ledger, got: {stdout}"
    );
    remove_temp_root(root);
}

/// `propose --write` regenerates the ledger from the loaded policy. Under the
/// no-policy fallback that policy was empty, so writing over an unusable
/// ledger destroyed every receipt it held (#1952).
#[test]
fn propose_write_preserves_a_ledger_it_cannot_read() {
    let root = temp_root("policy-discovery-propose-unusable");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/main.rs"),
        "fn main() { let v = vec![1]; println!(\"{}\", v[0]); }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    let ledger = root.join("policy/allow.toml");
    let original = "this is not valid toml {{{\nid = \"allow-9001\"\n";
    fs::write(&ledger, original)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write ledger: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["propose", "--write", "policy/allow.toml", "--force"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run propose: {err}")));

    assert_status("propose", &output, false);
    let written = fs::read_to_string(&ledger)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read ledger: {err}")));
    assert_eq!(
        written, original,
        "propose must not overwrite a ledger it could not read"
    );
    remove_temp_root(root);
}

/// The adoption path stays open: a tree with no policy candidate at all still
/// scans under the no-policy fallback.
#[test]
fn audit_still_scans_a_tree_with_no_policy_candidate() {
    let root = temp_root("policy-discovery-audit-unconfigured");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(root.join("src/main.rs"), "fn main() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["audit", "--format", "markdown"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));

    assert_status("audit", &output, true);
    remove_temp_root(root);
}
