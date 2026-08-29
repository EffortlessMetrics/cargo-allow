mod e2e_support;
mod json_assertions;
mod support;

use e2e_support::{assert_success_and_quiet, write_file};
use json_assertions::{assert_json_str, assert_json_u64};
use support::{
    assert_saved_json_artifact, assert_status, assert_stdout_empty, cargo_allow_command,
    remove_temp_root, temp_root,
};

#[test]
fn counted_baseline_entry_reports_occurrence_headroom_and_worklist_routing() {
    let root = temp_root("e2e-occurrence-headroom");
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
    // The fixture entry must stay live and non-review-due for the headroom
    // projection, and baseline_debt expiry is validated against the 120-day
    // window from creation, so its dates are computed relative to today
    // instead of hardcoded calendar days the test would eventually sail past.
    let created = allow_core::SimpleDate::today_utc_approx().add_days(-30);
    let review_after = allow_core::SimpleDate::today_utc_approx().add_days(30);
    let expires = allow_core::SimpleDate::today_utc_approx().add_days(60);
    write_file(
        &root,
        "policy/allow.toml",
        &format!(
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
classification = "baseline_debt"
reason = "Fixture caps crate manifests with counted baseline debt headroom."
occurrence_limit = 3
created = "{created}"
review_after = "{review_after}"
expires = "{expires}"
evidence = ["test:e2e_occurrence_headroom"]

[allow.selector]
ast_kind = "tracked_file"
target_fingerprint = "toml"
glob = "crates/*/Cargo.toml"
"#
        ),
    );

    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
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
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &check, true);
    assert_stdout_empty(
        "check",
        &check,
        "--receipt should not emit report JSON to stdout",
    );
    // #3190: with `--receipt` and no `--output`, the full report is withheld from
    // stdout but a one-line pass/fail summary is deliberately written to stderr so
    // the operator still gets a signal. Pin that rather than the old silence — the
    // property under test is that no *report* leaks, which the stdout check above
    // already covers.
    let check_stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        check_stderr.contains("cargo-allow check: passed (mode: no-new"),
        "headroom-only check should report its pass summary on stderr: `{check_stderr}`"
    );
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_json_u64(
        &receipt,
        "/advisory/occurrence_headroom",
        1,
        "receipt occurrence_headroom advisory count",
    );
    assert_json_str(
        &receipt,
        "/evidence_repair_queues/0/signal",
        "occurrence_headroom",
        "receipt routing signal",
    );
    assert_json_str(
        &receipt,
        "/evidence_repair_queues/0/item_kind",
        "occurrence_headroom",
        "receipt worklist item kind",
    );

    let worklist_output = root.join("target/cargo-allow/worklist.json");
    let worklist = cargo_allow_command()
        .arg("worklist")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--item-kind")
        .arg("occurrence_headroom")
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
        "one occurrence-headroom work item",
    );
    assert_json_str(
        &worklisted,
        "/work_items/0/kind",
        "occurrence_headroom",
        "worklist item kind",
    );
    assert_json_str(
        &worklisted,
        "/work_items/0/allow_id",
        "allow-crate-manifests",
        "worklist allow id",
    );
    let message = worklisted
        .pointer("/work_items/0/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("work item should include a message"));
    assert!(
        message.contains("occurrence_limit 3"),
        "unexpected headroom message: {message}"
    );
    assert!(
        message.contains("2 current matches"),
        "unexpected headroom message: {message}"
    );

    let deny = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("non-rust")
        .arg("--mode")
        .arg("no-new")
        .arg("--deny")
        .arg("occurrence_headroom")
        .output()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("run cargo-allow check --deny: {err}"))
        });
    assert_status("check --deny occurrence_headroom", &deny, false);

    remove_temp_root(root);
}
