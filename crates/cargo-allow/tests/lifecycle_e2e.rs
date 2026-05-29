mod support;

use std::fs;
use std::path::Path;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn cargo_prefix_init_propose_check_list_and_explain_round_trip() {
    let root = temp_root("lifecycle-e2e");
    write_source_fixture(&root);

    let policy = root.join("policy/allow.toml");
    let init = cargo_allow_command()
        .arg("allow")
        .arg("init")
        .arg("--strict")
        .arg("--config")
        .arg(&policy)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow allow init: {err}")));

    assert_status("allow init", &init, true);
    assert_stderr_empty("allow init", &init, "should not emit errors");
    assert_stdout_contains(
        "allow init",
        &init,
        "created",
        "cargo plugin prefix should initialize a policy file",
    );
    assert_file_contains(
        &policy,
        "default_mode = \"strict\"",
        "init --strict should persist strict defaults",
    );

    let proposed_policy = root.join("policy/allow.proposed.toml");
    let propose_summary = root.join("target/cargo-allow/propose-summary.json");
    let propose = cargo_allow_command()
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy)
        .arg("--include-untracked")
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&proposed_policy)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&propose_summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow propose: {err}")));

    assert_status("propose", &propose, true);
    assert_stdout_empty(
        "propose",
        &propose,
        "should not emit policy TOML when --write is used",
    );
    assert_stderr_empty(
        "propose",
        &propose,
        "should not emit summary text when --summary-output is used",
    );
    assert_file_contains(
        &proposed_policy,
        "id = \"allow-0001\"",
        "propose should add a baseline entry for the discovered panic finding",
    );
    assert_file_contains(
        &proposed_policy,
        "classification = \"baseline_debt\"",
        "propose should mark generated entries as baseline debt",
    );
    let summary = assert_saved_json_artifact(
        &propose_summary,
        "propose",
        "cargo-allow.propose.v1",
        "propose",
    );
    assert_json_u64(
        &summary,
        "/summary/findings_scanned",
        1,
        "propose finding count",
    );
    assert_json_u64(
        &summary,
        "/summary/baseline_debt_entries_proposed",
        1,
        "propose proposed entry count",
    );

    let check_report = root.join("target/cargo-allow/check.json");
    let check_receipt = root.join("target/cargo-allow/check.receipt.json");
    let check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&proposed_policy)
        .arg("--include-untracked")
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&check_report)
        .arg("--receipt")
        .arg(&check_receipt)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &check, true);
    assert_stdout_empty(
        "check",
        &check,
        "should not emit report JSON when --output is used",
    );
    assert_stderr_empty(
        "check",
        &check,
        "should not emit side-channel status when outputs are files",
    );
    let check_json =
        assert_saved_json_artifact(&check_report, "check", "cargo-allow.report.v1", "check");
    assert_json_bool(&check_json, "/failed", false, "check report failure flag");
    assert_json_u64(&check_json, "/summary/matched", 1, "check matched count");
    assert_saved_json_artifact(
        &check_receipt,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    let list = cargo_allow_command()
        .arg("list")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&proposed_policy)
        .arg("--include-untracked")
        .arg("--kind")
        .arg("panic")
        .arg("--baseline-debt")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow list: {err}")));

    assert_status("list", &list, true);
    assert_stderr_empty("list", &list, "should not emit errors");
    let list_json = parse_stdout_json("list", &list);
    assert_json_u64(&list_json, "/summary/allow_entries", 1, "list total count");
    assert_json_str(
        &list_json,
        "/allow_entries/0/id",
        "allow-0001",
        "list should expose the generated allow entry",
    );

    let explain = cargo_allow_command()
        .arg("explain")
        .arg("allow-0001")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&proposed_policy)
        .arg("--include-untracked")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow explain: {err}")));

    assert_status("explain", &explain, true);
    assert_stderr_empty("explain", &explain, "should not emit errors");
    let explain_json = parse_stdout_json("explain", &explain);
    assert_json_str(
        &explain_json,
        "/allow_entry/id",
        "allow-0001",
        "explain should resolve the generated allow entry",
    );
    assert_json_str(
        &explain_json,
        "/summary/current_status",
        "matched",
        "explain should show the generated entry matching the current finding",
    );

    remove_temp_root(root);
}

fn write_source_fixture(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
}

fn parse_stdout_json(command: &str, output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "{command} stdout should parse as JSON: {err}\n{}",
            String::from_utf8_lossy(&output.stdout)
        ))
    })
}

fn assert_stdout_contains(
    command: &str,
    result: &std::process::Output,
    needle: &str,
    message: &str,
) {
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains(needle), "{command} {message}: `{stdout}`");
}

fn assert_file_contains(path: &Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}

fn assert_json_u64(value: &Value, pointer: &str, expected: u64, label: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_u64),
        Some(expected),
        "{label}"
    );
}

fn assert_json_bool(value: &Value, pointer: &str, expected: bool, label: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_bool),
        Some(expected),
        "{label}"
    );
}

fn assert_json_str(value: &Value, pointer: &str, expected: &str, label: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_str),
        Some(expected),
        "{label}"
    );
}
