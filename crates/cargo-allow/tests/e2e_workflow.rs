mod support;

use std::fs;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn cargo_alias_init_add_and_strict_check_cover_full_policy_workflow() {
    let root = temp_root("e2e-policy-workflow");
    let policy_path = root.join("policy/allow.toml");

    let init = cargo_allow_command()
        .arg("allow")
        .arg("init")
        .arg("--strict")
        .arg("--config")
        .arg(&policy_path)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow allow init: {err}")));
    assert_status("allow init", &init, true);
    assert_stderr_empty("allow init", &init, "should not emit errors");
    assert_stdout_contains(
        "allow init",
        &init,
        "created",
        "should confirm policy creation",
    );
    assert_file_contains(
        &policy_path,
        "default_mode = \"strict\"",
        "strict init should persist strict defaults",
    );

    write_panic_source(&root);

    let before = run_strict_panic_check_json(&root);
    assert_status("check before add", &before, false);
    assert_stderr_empty(
        "check before add",
        &before,
        "JSON report should stay on stdout",
    );
    let before_json = stdout_json("check before add", &before);
    assert_json_string(&before_json, "/status", "failed", "check status");
    assert_json_bool(&before_json, "/failed", true, "check failed flag");
    assert_json_outcome_status(&before_json, "new");

    let add_summary_path = root.join("target/cargo-allow/add-summary.json");
    let add = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_path)
        .arg("--kind")
        .arg("panic")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--owner")
        .arg("core")
        .arg("--reason")
        .arg("fixture")
        .arg("--write")
        .arg(&policy_path)
        .arg("--force")
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&add_summary_path)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow add: {err}")));
    assert_status("add", &add, true);
    assert_stdout_empty("add", &add, "--write should keep policy TOML out of stdout");
    assert_stderr_empty(
        "add",
        &add,
        "--summary-output should keep summary out of stderr",
    );
    assert_file_contains(
        &policy_path,
        "id = \"allow-0001\"",
        "add should append a generated allow entry id",
    );
    let add_summary = assert_saved_json_artifact(
        &add_summary_path,
        "add summary",
        "cargo-allow.add.v1",
        "add",
    );
    assert_json_string(
        &add_summary,
        "/summary/entry_id",
        "allow-0001",
        "add summary entry id",
    );
    assert_json_string(
        &add_summary,
        "/allow_entry/id",
        "allow-0001",
        "add allow entry id",
    );

    let after = run_strict_panic_check_json(&root);
    assert_status("check after add", &after, true);
    assert_stderr_empty(
        "check after add",
        &after,
        "JSON report should stay on stdout",
    );
    let after_json = stdout_json("check after add", &after);
    assert_json_string(&after_json, "/status", "passed", "check status");
    assert_json_bool(&after_json, "/failed", false, "check failed flag");
    assert_json_outcome_status(&after_json, "matched");

    remove_temp_root(root);
}

#[test]
fn init_refuses_to_overwrite_existing_policy_without_force() {
    let root = temp_root("e2e-init-overwrite");
    let policy_path = root.join("policy/allow.toml");

    let first = cargo_allow_command()
        .arg("init")
        .arg("--config")
        .arg(&policy_path)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run first init: {err}")));
    assert_status("first init", &first, true);

    let second = cargo_allow_command()
        .arg("init")
        .arg("--config")
        .arg(&policy_path)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run second init: {err}")));
    assert_status("second init", &second, false);
    assert_stdout_empty(
        "second init",
        &second,
        "failed init should not print success",
    );
    assert_stderr_contains(
        "second init",
        &second,
        "already exists; use --force to overwrite",
        "should explain how to overwrite the existing policy",
    );

    let forced = cargo_allow_command()
        .arg("init")
        .arg("--strict")
        .arg("--force")
        .arg("--config")
        .arg(&policy_path)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run forced init: {err}")));
    assert_status("forced init", &forced, true);
    assert_file_contains(
        &policy_path,
        "default_mode = \"strict\"",
        "forced init should replace the existing policy",
    );

    remove_temp_root(root);
}

fn run_strict_panic_check_json(root: &std::path::Path) -> std::process::Output {
    cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(root)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("strict")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")))
}

fn write_panic_source(root: &std::path::Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write panic source: {err}")));
}

fn stdout_json(command: &str, output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "{command} stdout should parse as JSON: {err}\n{}",
            String::from_utf8_lossy(&output.stdout)
        ))
    })
}

fn assert_json_string(value: &Value, pointer: &str, expected: &str, label: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_str),
        Some(expected),
        "{label} at {pointer}"
    );
}

fn assert_json_bool(value: &Value, pointer: &str, expected: bool, label: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_bool),
        Some(expected),
        "{label} at {pointer}"
    );
}

fn assert_json_outcome_status(value: &Value, expected: &str) {
    let outcomes = value
        .get("outcomes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("report outcomes should be an array"));
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.get("status").and_then(Value::as_str) == Some(expected)),
        "report outcomes should contain status {expected}: {outcomes:?}"
    );
}

fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
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

fn assert_stderr_contains(
    command: &str,
    output: &std::process::Output,
    needle: &str,
    message: &str,
) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(needle), "{command} {message}: `{stderr}`");
}
