mod support;

use std::fs;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn check_receipt_file_exposes_saved_json_contract() {
    let root = temp_root("receipt-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.md");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("markdown")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report markdown to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    remove_temp_root(root);
}

#[test]
fn check_success_reports_policy_missing_evidence_counts() {
    let root = temp_root("receipt-policy-missing-evidence");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/policy.md"), "# Policy\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write doc fixture: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_missing_evidence(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "passed", "report status");
    assert_json_u64(
        &report,
        "/summary/policy_missing_evidence",
        1,
        "report summary policy_missing_evidence",
    );
    assert_json_u64(
        &report,
        "/trend/policy_missing_evidence",
        1,
        "report trend policy_missing_evidence",
    );
    assert_json_str(&receipt, "/status", "passed", "receipt status");
    assert_json_u64(
        &receipt,
        "/counts/policy_missing_evidence",
        1,
        "receipt policy_missing_evidence",
    );

    remove_temp_root(root);
}

#[test]
fn check_failure_with_broken_evidence_still_writes_report_and_receipt() {
    assert_check_failure_reports_broken_evidence(
        "receipt-broken-evidence",
        policy_with_broken_evidence(),
    );
}

#[test]
fn check_failure_with_invalid_evidence_scope_still_writes_report_and_receipt() {
    assert_check_failure_reports_broken_evidence(
        "receipt-invalid-evidence-scope",
        policy_with_escaping_evidence(),
    );
}

fn assert_check_failure_reports_broken_evidence(fixture: &str, policy: &str) {
    let root = temp_root(fixture);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "failed", "report status");
    assert_json_u64(
        &report,
        "/summary/broken_evidence_links",
        1,
        "report summary broken_evidence_links",
    );
    assert_json_u64(
        &report,
        "/trend/broken_evidence_links",
        1,
        "report trend broken_evidence_links",
    );
    assert_json_str(&receipt, "/status", "failed", "receipt status");
    assert_json_u64(
        &receipt,
        "/counts/broken_evidence_links",
        1,
        "receipt broken_evidence_links",
    );

    remove_temp_root(root);
}

fn policy() -> &'static str {
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

fn policy_with_missing_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["test:check_success_reports_policy_missing_evidence_counts"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"

[[allow]]
id = "allow-doc"
kind = "non_rust_file"
family = "documentation"
path = "docs/policy.md"
owner = "core"
classification = "fixture"
reason = "fixture policy documentation"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/policy.md"
target_fingerprint = "md"
glob = "docs/policy.md"
"#
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

fn policy_with_escaping_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["doc:../outside.md"]
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

fn assert_json_str(value: &Value, pointer: &str, expected: &str, message: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_str),
        Some(expected),
        "{message}"
    );
}
