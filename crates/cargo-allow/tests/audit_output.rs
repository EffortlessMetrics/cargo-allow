mod support;

use std::fs;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn audit_with_output_file_does_not_emit_human_status_to_stderr() {
    let root = temp_root("audit-output");
    fs::write(root.join("tracked.txt"), "tracked\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");

    remove_temp_root(root);
}

#[test]
fn audit_with_broken_evidence_writes_saved_report_counts() {
    let root = temp_root("audit-broken-evidence-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_broken_evidence(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_u64(
        &report,
        "/summary/broken_evidence_links",
        1,
        "audit summary broken_evidence_links",
    );
    assert_json_u64(
        &report,
        "/trend/broken_evidence_links",
        1,
        "audit trend broken_evidence_links",
    );

    remove_temp_root(root);
}

fn policy_with_broken_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["doc:docs/missing-evidence.md"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}

fn assert_json_u64(value: &Value, pointer: &str, expected: u64, message: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_u64),
        Some(expected),
        "{message}"
    );
}
