mod support;

use std::fs;

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
