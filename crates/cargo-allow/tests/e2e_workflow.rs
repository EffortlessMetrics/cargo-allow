mod support;

use std::fs;
use std::process::Output;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn propose_list_and_check_round_trip_with_cargo_prefix() {
    let root = temp_root("e2e-round-trip");
    write_panic_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    let policy_output = root.join("policy/allow.toml");
    let propose_summary = root.join("target/cargo-allow/propose.json");
    let propose = cargo_allow_command()
        .arg("allow")
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&propose_summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow propose: {err}")));

    assert_success_and_quiet("propose", &propose);
    let proposed = assert_saved_json_artifact(
        &propose_summary,
        "propose",
        "cargo-allow.propose.v1",
        "propose",
    );
    assert_json_u64(
        &proposed,
        "/summary/findings_scanned",
        1,
        "propose scanned one panic finding",
    );
    assert_json_u64(
        &proposed,
        "/summary/baseline_debt_entries_proposed",
        1,
        "propose generated one baseline entry",
    );
    assert_file_contains(
        &policy_output,
        "classification = \"baseline_debt\"",
        "propose should write baseline-debt policy entries",
    );

    let list_output = root.join("target/cargo-allow/list.json");
    let list = cargo_allow_command()
        .arg("allow")
        .arg("list")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_output)
        .arg("--kind")
        .arg("panic")
        .arg("--status")
        .arg("baseline_debt")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&list_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow list: {err}")));

    assert_success_and_quiet("list", &list);
    let listed = assert_saved_json_artifact(&list_output, "list", "cargo-allow.list.v1", "list");
    assert_json_u64(
        &listed,
        "/summary/allow_entries",
        1,
        "list should return the generated baseline entry",
    );
    assert_json_str(
        &listed,
        "/allow_entries/0/status",
        "baseline_debt",
        "list should expose the baseline-debt status",
    );

    let check_output = root.join("target/cargo-allow/check.json");
    let check = cargo_allow_command()
        .arg("allow")
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_output)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&check_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_success_and_quiet("check", &check);
    let checked =
        assert_saved_json_artifact(&check_output, "check", "cargo-allow.report.v1", "check");
    assert_json_str(&checked, "/status", "passed", "check should pass");
    assert_json_u64(&checked, "/summary/matched", 1, "check matched count");
    assert_json_u64(&checked, "/summary/new", 0, "check new count");

    remove_temp_root(root);
}

#[test]
fn check_reports_new_findings_after_policy_baseline_is_outdated() {
    let root = temp_root("e2e-new-finding");
    write_panic_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    let policy_output = root.join("policy/allow.toml");
    let propose_summary = root.join("target/cargo-allow/propose.json");
    let propose = cargo_allow_command()
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&propose_summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow propose: {err}")));
    assert_success_and_quiet("propose", &propose);

    write_panic_source(
        &root,
        concat!(
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
            "pub fn reload(value: Result<u8, ()>) -> u8 { value.unwrap() }\n",
        ),
    );

    let check_output = root.join("target/cargo-allow/check-new.json");
    let check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_output)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&check_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &check, false);
    assert_stdout_empty(
        "check",
        &check,
        "--output should not emit failure report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &check,
        "--output should not emit human failure status to stderr",
    );
    let checked =
        assert_saved_json_artifact(&check_output, "check", "cargo-allow.report.v1", "check");
    assert_json_str(&checked, "/status", "failed", "check should fail");
    assert_json_u64(&checked, "/summary/matched", 1, "check matched count");
    assert_json_u64(&checked, "/summary/new", 1, "check new count");
    assert_json_str(
        &checked,
        "/outcomes/1/status",
        "new",
        "check should report the added panic finding as new",
    );

    remove_temp_root(root);
}

fn assert_success_and_quiet(command: &str, result: &Output) {
    assert_status(command, result, true);
    assert_stdout_empty(
        command,
        result,
        "should not emit primary output when output files are configured",
    );
    assert_stderr_empty(
        command,
        result,
        "should not emit side-channel status when output files are configured",
    );
}

fn write_panic_source(root: &std::path::Path, contents: &str) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(root.join("src/lib.rs"), contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
}

fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}

fn assert_json_u64(value: &Value, pointer: &str, expected: u64, message: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_u64),
        Some(expected),
        "{message}"
    );
}

fn assert_json_str(value: &Value, pointer: &str, expected: &str, message: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_str),
        Some(expected),
        "{message}"
    );
}
