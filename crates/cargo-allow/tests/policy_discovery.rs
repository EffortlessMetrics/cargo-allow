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
        .unwrap_or_else(|err| panic!("system clock: {err}"))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove temp root {}: {err}", root.display()),
    }
}

fn write_foreign_allow_toml(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::write(
        root.join("policy/allow.toml"),
        r#"
schema_version = "1"
owner = "repo-policy"
"#,
    )
    .unwrap_or_else(|err| panic!("write foreign allow.toml: {err}"));
}

fn write_native_ledger(root: &std::path::Path) {
    fs::write(
        root.join("policy/cargo-allow.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )
    .unwrap_or_else(|err| panic!("write native cargo-allow.toml: {err}"));
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
        .unwrap_or_else(|err| panic!("run list: {err}"));

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
        .unwrap_or_else(|err| panic!("run list: {err}"));

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
    .unwrap_or_else(|err| panic!("write explicit policy: {err}"));

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
        .unwrap_or_else(|err| panic!("run list: {err}"));

    assert_status("list", &output, true);
    remove_temp_root(root);
}
