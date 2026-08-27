mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn init_add_check_explain_and_prune_work_as_a_cli_lifecycle() {
    let root = temp_root("e2e-lifecycle");
    write_source_fixture(&root);
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);

    let init = cargo_allow_command()
        .current_dir(&root)
        .arg("init")
        .arg("--strict")
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow init: {err}"));
    assert_status("init", &init, true);
    assert_stderr_empty("init", &init, "should not emit errors on successful init");
    assert_stdout_contains(
        "init",
        &init,
        "created policy/allow.toml",
        "should report the created default policy path relative to the working directory",
    );
    assert_file_contains(
        &root.join("policy/allow.toml"),
        "policy = \"cargo-allow\"",
        "init should create a cargo-allow policy",
    );

    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial fixture"]);

    let failing_check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| {
            panic!("run cargo-allow check before add: {err}")
        });
    assert_status("check before add", &failing_check, false);
    assert_stderr_empty(
        "check before add",
        &failing_check,
        "expected policy failures should be represented in the JSON report, not stderr",
    );
    let failing_report = parse_stdout_json("check before add", &failing_check);
    assert_json_artifact_header(
        &failing_report,
        "check before add",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        failing_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "check before add should fail the no-new gate"
    );
    assert_eq!(
        failing_report.pointer("/trend/new").and_then(Value::as_u64),
        Some(1),
        "check before add should report the unwrap as a new finding"
    );

    let add_summary = root.join("target/cargo-allow/add-summary.json");
    let add = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--id")
        .arg("allow-e2e-panic")
        .arg("--owner")
        .arg("core")
        .arg("--reason")
        .arg("The fixture intentionally unwraps after callers provide Some values.")
        .arg("--evidence")
        .arg("test:e2e_lifecycle")
        .arg("--write")
        .arg(root.join("policy/allow.toml"))
        .arg("--force")
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&add_summary)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow add: {err}"));
    assert_status("add", &add, true);
    assert_stdout_empty(
        "add",
        &add,
        "--write should not emit the updated policy to stdout",
    );
    assert_stderr_empty(
        "add",
        &add,
        "--summary-output should not emit the summary to stderr",
    );
    assert_saved_json_artifact(&add_summary, "add", "cargo-allow.add.v1", "add");
    assert_file_contains(
        &root.join("policy/allow.toml"),
        "id = \"allow-e2e-panic\"",
        "add should persist the selected finding as a policy entry",
    );

    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-m", "allow panic fixture"]);

    let check_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let passing_check = cargo_allow_command()
        .arg("allow")
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&check_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow allow check: {err}"));
    assert_status("cargo allow check", &passing_check, true);
    assert_stdout_empty(
        "cargo allow check",
        &passing_check,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "cargo allow check",
        &passing_check,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let passing_report =
        assert_saved_json_artifact(&check_output, "check", "cargo-allow.report.v1", "check");
    assert_eq!(
        passing_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "check after add should pass the no-new gate"
    );
    assert_eq!(
        passing_report.pointer("/trend/new").and_then(Value::as_u64),
        Some(0),
        "check after add should not leave new panic findings"
    );
    assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    let explain_output = root.join("target/cargo-allow/explain.json");
    let explain = cargo_allow_command()
        .arg("explain")
        .arg("allow-e2e-panic")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&explain_output)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow explain: {err}"));
    assert_status("explain", &explain, true);
    assert_stdout_empty(
        "explain",
        &explain,
        "--output should not emit explanation JSON to stdout",
    );
    assert_stderr_empty(
        "explain",
        &explain,
        "--output should not emit side-channel status to stderr",
    );
    let explanation = assert_saved_json_artifact(
        &explain_output,
        "explain",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_eq!(
        explanation
            .pointer("/allow_entry/id")
            .and_then(Value::as_str),
        Some("allow-e2e-panic"),
        "explain should select the newly added allow entry"
    );

    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap_or_default() }\n",
    )
    .unwrap_or_else(|err| panic!("remove unwrap fixture: {err}"));
    git(&root, &["add", "src/lib.rs"]);
    git(&root, &["commit", "-m", "remove panic fixture"]);

    let prune_output = root.join("target/cargo-allow/prune.json");
    let prune = cargo_allow_command()
        .arg("prune")
        .arg("--root")
        .arg(&root)
        .arg("--stale")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&prune_output)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow prune: {err}"));
    assert_status("prune", &prune, true);
    assert_stdout_empty(
        "prune",
        &prune,
        "--output should not emit prune JSON to stdout",
    );
    assert_stderr_empty(
        "prune",
        &prune,
        "--output should not emit side-channel status to stderr",
    );
    let prune_report =
        assert_saved_json_artifact(&prune_output, "prune", "cargo-allow.prune.v1", "prune");
    assert_eq!(
        prune_report
            .pointer("/summary/stale_entries")
            .and_then(Value::as_u64),
        Some(1),
        "prune should surface the allow entry after its finding disappears"
    );
    assert_prune_contains_stale_id(&prune_report, "allow-e2e-panic");

    remove_temp_root(root);
}

fn write_source_fixture(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create source dir: {err}"));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write source fixture: {err}"));
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git {args:?}: {err}"));
    if !output.status.success() {
        panic!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn parse_stdout_json(command: &str, output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "{command} stdout should parse as JSON: {err}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn assert_json_artifact_header(
    value: &Value,
    name: &str,
    expected_schema_id: &str,
    expected_command: &str,
) {
    assert_eq!(
        value.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "{name} schema_version"
    );
    assert_eq!(
        value.get("schema_id").and_then(Value::as_str),
        Some(expected_schema_id),
        "{name} schema_id"
    );
    assert_eq!(
        value.get("tool").and_then(Value::as_str),
        Some("cargo-allow"),
        "{name} tool"
    );
    assert_eq!(
        value.get("command").and_then(Value::as_str),
        Some(expected_command),
        "{name} command"
    );
}

fn assert_stdout_contains(
    command: &str,
    output: &std::process::Output,
    needle: &str,
    message: &str,
) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(needle), "{command} {message}: `{stdout}`");
}

fn assert_file_contains(path: &Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(contents.contains(needle), "{message}");
}

fn assert_prune_contains_stale_id(report: &Value, expected_id: &str) {
    let stale_entries = report
        .get("stale_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("prune report should contain stale_entries"));
    assert!(
        stale_entries
            .iter()
            .any(|entry| entry.get("id").and_then(Value::as_str) == Some(expected_id)),
        "prune report should include stale entry {expected_id}: {stale_entries:?}"
    );
}
