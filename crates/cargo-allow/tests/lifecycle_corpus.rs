mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

const EXPIRED_ID: &str = "allow-expired";
const REVIEW_DUE_ID: &str = "allow-review";

#[test]
fn lifecycle_statuses_converge_across_read_artifacts() {
    let root = create_fixture("lifecycle-corpus", true);

    let (list_path, list_result) = run_report(&root, "list", &["list"]);
    assert_status("list", &list_result, true);
    assert_quiet("list", &list_result);
    let list = assert_saved_json_artifact(&list_path, "list", "cargo-allow.list.v1", "list");
    assert_entry_status(&list, "/allow_entries", EXPIRED_ID, "expired");
    assert_entry_status(&list, "/allow_entries", REVIEW_DUE_ID, "review_due");

    let (expired_path, expired_result) =
        run_report(&root, "explain-expired", &["explain", EXPIRED_ID]);
    assert_status("explain expired", &expired_result, true);
    assert_quiet("explain expired", &expired_result);
    let expired = assert_saved_json_artifact(
        &expired_path,
        "explain expired",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&expired, EXPIRED_ID, "expired");

    let (review_path, review_result) =
        run_report(&root, "explain-review", &["explain", REVIEW_DUE_ID]);
    assert_status("explain review", &review_result, true);
    assert_quiet("explain review", &review_result);
    let review = assert_saved_json_artifact(
        &review_path,
        "explain review",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&review, REVIEW_DUE_ID, "review_due");

    let (worklist_path, worklist_result) = run_report(&root, "worklist", &["worklist"]);
    assert_status("worklist", &worklist_result, true);
    assert_quiet("worklist", &worklist_result);
    let worklist = assert_saved_json_artifact(
        &worklist_path,
        "worklist",
        "cargo-allow.worklist.v1",
        "worklist",
    );
    assert_entry_status(&worklist, "/work_items", EXPIRED_ID, "expired");
    assert_entry_status(&worklist, "/work_items", REVIEW_DUE_ID, "review_due");

    for (command, args, should_succeed) in [
        ("audit", vec!["audit"], true),
        ("check", vec!["check", "--mode", "no-new"], false),
        ("diff", vec!["diff", "--base", "HEAD"], false),
    ] {
        let (path, result) = run_report(&root, command, &args);
        assert_status(command, &result, should_succeed);
        assert_quiet(command, &result);
        let report = assert_saved_json_artifact(&path, command, "cargo-allow.report.v1", command);
        assert_entry_status(&report, "/outcomes", EXPIRED_ID, "expired");
        assert_entry_status(&report, "/outcomes", REVIEW_DUE_ID, "review_due");
    }

    remove_temp_root(root);
}

#[test]
fn review_due_is_advisory_in_no_new_and_blocking_in_strict() {
    let root = create_fixture("review-due-mode", false);

    let no_new_output = root.join("target/cargo-allow/no-new.json");
    let no_new = run_command(&root, &["check", "--mode", "no-new"], &no_new_output);
    assert_status("review-due no-new", &no_new, true);
    assert_quiet("review-due no-new", &no_new);
    let no_new_report = assert_saved_json_artifact(
        &no_new_output,
        "review-due no-new",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        no_new_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "review_due is advisory in no-new mode"
    );
    assert_entry_status(&no_new_report, "/outcomes", REVIEW_DUE_ID, "review_due");

    let strict_output = root.join("target/cargo-allow/strict.json");
    let strict = run_command(&root, &["check", "--mode", "strict"], &strict_output);
    assert_status("review-due strict", &strict, false);
    assert_quiet("review-due strict", &strict);
    let strict_report = assert_saved_json_artifact(
        &strict_output,
        "review-due strict",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        strict_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "review_due is blocking in strict mode"
    );
    assert_entry_status(&strict_report, "/outcomes", REVIEW_DUE_ID, "review_due");

    remove_temp_root(root);
}

fn create_fixture(label: &str, include_expired: bool) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source directory: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy directory: {err}")));
    let source = if include_expired {
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\npub fn reload(value: Option<u8>) -> u8 { value.unwrap() }\n"
    } else {
        "pub fn reload(value: Option<u8>) -> u8 { value.unwrap() }\n"
    };
    fs::write(root.join("src/lib.rs"), source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
    fs::write(root.join("policy/allow.toml"), policy(include_expired))
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy fixture: {err}")));

    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "lifecycle corpus fixture"]);
    root
}

fn policy(include_expired: bool) -> String {
    let expired = if include_expired {
        format!(
            r#"
[[allow]]
id = "{EXPIRED_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture intentionally unwraps after callers provide Some values."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
expires = "2020-01-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
        )
    } else {
        String::new()
    };
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
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
{expired}
[[allow]]
id = "{REVIEW_DUE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture intentionally unwraps after callers provide Some values."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2020-01-01"

[allow.selector]
ast_kind = "method_call"
container = "reload"
callee = "unwrap"
"#
    )
}

fn run_report(root: &Path, name: &str, args: &[&str]) -> (PathBuf, Output) {
    let output = root.join(format!("target/cargo-allow/{name}.json"));
    let result = run_command(root, args, &output);
    (output, result)
}

fn run_command(root: &Path, args: &[&str], output: &Path) -> Output {
    cargo_allow_command()
        .args(args)
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")))
}

fn assert_quiet(command: &str, result: &Output) {
    assert_stdout_empty(command, result, "--output should not emit report JSON");
    assert_stderr_empty(command, result, "--output should not emit status text");
}

fn assert_explain_status(value: &Value, allow_id: &str, status: &str) {
    assert_eq!(
        value.pointer("/allow_entry/id").and_then(Value::as_str),
        Some(allow_id),
        "explain should retain the allow ID"
    );
    assert_eq!(
        value
            .pointer("/summary/current_status")
            .and_then(Value::as_str),
        Some(status),
        "{allow_id} explain status"
    );
}

fn assert_entry_status(value: &Value, collection_pointer: &str, allow_id: &str, status: &str) {
    let entries = value
        .pointer(collection_pointer)
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{collection_pointer} should be an array"))
        });
    let entry = entries
        .iter()
        .find(|entry| {
            entry.get("allow_id").and_then(Value::as_str) == Some(allow_id)
                || entry.get("id").and_then(Value::as_str) == Some(allow_id)
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "{allow_id} missing from {collection_pointer}: {entries:?}"
            ))
        });
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some(status),
        "{allow_id} status in {collection_pointer}"
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}
