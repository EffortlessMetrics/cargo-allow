//! Real-binary exit-code matrix for #2340.
//!
//! Proves process exit classes without calling the mapping helper:
//! Clap usage and structured `Usage` → 2; config/policy/runtime → 1; success → 0.
//!
//! Focused test: self-contained subprocess helpers (no shared `tests/support`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cargo_allow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-exit-matrix-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn drop_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}

fn assert_exit(label: &str, output: &Output, expected: i32, stderr_fragment: &str) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{label}: expected exit {expected}, got {:?}; stdout=`{}` stderr=`{}`",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(stderr_fragment),
        "{label}: stderr should contain `{stderr_fragment}`; got `{stderr}`"
    );
}

fn assert_success(label: &str, output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{label}: expected exit 0, got {:?}; stdout=`{}` stderr=`{}`",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_panic_source(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write src/lib.rs: {err}")));
}

fn write_empty_policy(root: &Path) -> PathBuf {
    let policy_dir = root.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let path = policy_dir.join("allow.toml");
    fs::write(&path, "policy = \"cargo-allow\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write empty policy: {err}")));
    path
}

fn fixture_policy() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}

#[test]
fn exit_matrix_unknown_clap_flag_is_2() {
    let output = cargo_allow()
        .arg("doctor")
        .arg("--not-a-real-flag")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run unknown flag: {err}")));
    assert_exit(
        "unknown clap flag",
        &output,
        2,
        "unexpected argument '--not-a-real-flag'",
    );
}

#[test]
fn exit_matrix_missing_required_clap_value_is_2() {
    let output = cargo_allow()
        .arg("check")
        .arg("--mode")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run missing mode value: {err}")));
    assert_exit("missing required clap value", &output, 2, "--mode");
}

#[test]
fn exit_matrix_clap_conflicting_list_status_shortcuts_is_2() {
    let root = temp_root("exit-list-conflict");
    write_empty_policy(&root);
    let output = cargo_allow()
        .arg("list")
        .arg("--root")
        .arg(&root)
        .arg("--expired")
        .arg("--stale")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run list conflict: {err}")));
    assert_exit("list --expired --stale", &output, 2, "--expired");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--stale"),
        "stderr should identify both shortcuts: {stderr}"
    );
    drop_root(root);
}

#[test]
fn exit_matrix_post_parse_structured_usage_is_2() {
    // This test exists to prove the *post-parse* route: a structured
    // `CargoAllowErrorKind::Usage` raised by our own code after clap has
    // accepted the argv still exits 2. It used to ride on `add --glob` with
    // `--path`, but #3203/#3210 moved that exclusion into clap's
    // `conflicts_with`, so that combination is now rejected at parse time and
    // no longer exercises this path. `--command-summary-output` on a command
    // that does not emit the summary is parse-valid and rejected afterwards,
    // so it exercises the same route this test was written for.
    let root = temp_root("exit-post-parse-usage");
    write_panic_source(&root);
    write_empty_policy(&root);
    let summary = root.join("summary.json");
    let output = cargo_allow()
        .arg("--command-summary-output")
        .arg(&summary)
        .arg("vocabulary")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run post-parse usage: {err}")));
    assert_exit(
        "post-parse structured Usage (--command-summary-output on an unsupported command)",
        &output,
        2,
        "currently supports the source-exception",
    );
    assert!(
        !summary.exists(),
        "a rejected usage error must not leave a partial artifact behind"
    );
    drop_root(root);
}

#[test]
fn exit_matrix_command_summary_output_without_a_subcommand_is_2() {
    // `--command-summary-output` is a global flag, so it parses fine with no
    // subcommand at all. That reaches a second post-parse `Usage` branch,
    // distinct from the unsupported-subcommand one above.
    let root = temp_root("exit-summary-no-subcommand");
    let summary = root.join("summary.json");
    let output = cargo_allow()
        .arg("--command-summary-output")
        .arg(&summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run bare usage: {err}")));
    assert_exit(
        "post-parse structured Usage (--command-summary-output with no subcommand)",
        &output,
        2,
        "requires the adopt, doctor, audit, check, diff, init, propose, explain, why, or worklist subcommand",
    );
    assert!(
        !summary.exists(),
        "a rejected usage error must not leave a partial artifact behind"
    );
    drop_root(root);
}

#[test]
fn exit_matrix_missing_config_is_1() {
    let root = temp_root("exit-missing-config");
    write_panic_source(&root);
    let missing = root.join("policy/does-not-exist.toml");
    let output = cargo_allow()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .arg("--config")
        .arg(&missing)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run missing config: {err}")));
    assert_exit("missing config", &output, 1, "error[");
    drop_root(root);
}

#[test]
fn exit_matrix_invalid_policy_content_is_1() {
    let root = temp_root("exit-invalid-policy");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let invalid = format!(
        r#"{}

[[allow]]
id = "allow-invalid"
kind = "panic"
family = "unwrap"
path = "src/invalid.rs"
owner = "core"
classification = "fixture"
reason = "fixture invalid lifecycle"
created = "2026-08-01"
review_after = "2026-07-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
        fixture_policy()
    );
    fs::write(root.join("policy/allow.toml"), invalid)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid policy: {err}")));
    let output = cargo_allow()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run invalid policy: {err}")));
    assert_exit("invalid policy content", &output, 1, "review_after");
    drop_root(root);
}

#[test]
fn exit_matrix_check_policy_violation_is_1() {
    let root = temp_root("exit-policy-violation");
    write_panic_source(&root);
    write_empty_policy(&root);
    let output = cargo_allow()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("strict")
        .arg("--kind")
        .arg("panic")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run strict check: {err}")));
    assert_eq!(
        output.status.code(),
        Some(1),
        "strict policy violation should exit 1; stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop_root(root);
}

#[test]
fn exit_matrix_successful_doctor_is_0() {
    let root = temp_root("exit-doctor-ok");
    write_empty_policy(&root);
    let output = cargo_allow()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}")));
    assert_success("doctor success", &output);
    drop_root(root);
}
