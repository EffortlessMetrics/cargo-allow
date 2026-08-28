mod e2e_support;
mod json_assertions;
mod support;

use e2e_support::{assert_success_and_quiet, write_file};
use json_assertions::{assert_json_str, assert_json_u64};
use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

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
