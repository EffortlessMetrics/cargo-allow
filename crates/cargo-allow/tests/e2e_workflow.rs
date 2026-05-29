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

#[test]
fn capped_manifest_policy_reports_extra_crate_manifest_as_new_debt() {
    let root = temp_root("e2e-manifest-occurrence-limit");
    write_file(
        &root,
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    );
    write_file(
        &root,
        "crates/beta/Cargo.toml",
        "[package]\nname = \"beta\"\nversion = \"0.1.0\"\n",
    );
    write_file(
        &root,
        "policy/allow.toml",
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

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-crate-manifests"
kind = "non_rust_file"
family = "package_metadata"
glob = "crates/*/Cargo.toml"
owner = "core/release"
classification = "rust_package_metadata"
reason = "Fixture caps current crate manifests so added manifests become new debt."
occurrence_limit = 1
review_after = "2026-08-29"

[allow.selector]
ast_kind = "tracked_file"
target_fingerprint = "toml"
glob = "crates/*/Cargo.toml"
"#,
    );

    let check_output = root.join("target/cargo-allow/check.json");
    let check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("non-rust")
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
    assert_json_u64(&checked, "/summary/matched", 1, "one manifest matched");
    assert_json_u64(&checked, "/summary/new", 1, "one manifest exceeded the cap");
    assert_json_str(
        &checked,
        "/outcomes/1/status",
        "new",
        "occurrence-limit overage should be reported as new debt",
    );
    let message = checked
        .pointer("/outcomes/1/message")
        .and_then(Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("overage outcome should have a message"));
    assert!(
        message.contains("occurrence_limit exceeded"),
        "unexpected overage message: {message}"
    );

    let worklist_output = root.join("target/cargo-allow/worklist.json");
    let worklist = cargo_allow_command()
        .arg("worklist")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--item-kind")
        .arg("occurrence_limit_exceeded")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&worklist_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow worklist: {err}")));

    assert_success_and_quiet("worklist", &worklist);
    let worklisted = assert_saved_json_artifact(
        &worklist_output,
        "worklist",
        "cargo-allow.worklist.v1",
        "worklist",
    );
    assert_json_u64(
        &worklisted,
        "/summary/work_items",
        1,
        "one occurrence-limit work item",
    );
    assert_json_str(
        &worklisted,
        "/work_items/0/kind",
        "occurrence_limit_exceeded",
        "worklist item kind",
    );
    assert_json_str(
        &worklisted,
        "/work_items/0/allow_id",
        "allow-crate-manifests",
        "worklist allow id",
    );
    assert_json_str(
        &worklisted,
        "/work_items/0/path",
        "crates/beta/Cargo.toml",
        "worklist path should point at the excess manifest",
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

fn write_file(root: &std::path::Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap_or_else(|| {
        std::panic::panic_any(format!("fixture path has no parent: {}", path.display()))
    }))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("create fixture parent {}: {err}", path.display()))
    });
    fs::write(&path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {}: {err}", path.display())));
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
