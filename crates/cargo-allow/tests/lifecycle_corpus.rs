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
const STALE_ID: &str = "allow-stale";
const DRIFT_ID: &str = "allow-drift";
const HEADROOM_ID: &str = "allow-headroom";
const MISSING_EVIDENCE_ID: &str = "allow-missing-evidence";
const BROKEN_EVIDENCE_ID: &str = "allow-broken-evidence";
const WEAK_EVIDENCE_ID: &str = "allow-weak-evidence";
const AUDIT_ARGS: &[&str] = &["audit"];
const CHECK_NO_NEW_ARGS: &[&str] = &["check", "--mode", "no-new"];
const DIFF_ARGS: &[&str] = &["diff", "--base", "HEAD"];

#[test]
fn lifecycle_statuses_converge_across_read_artifacts() {
    let root = create_fixture("lifecycle-corpus", true);

    let (list_path, list_result) = run_report(&root, "list", &["list"]);
    assert_status("list", &list_result, true);
    assert_quiet("list", &list_result);
    let list = assert_saved_json_artifact(&list_path, "list", "cargo-allow.list.v1", "list");
    assert_entry_status(&list, "/allow_entries", EXPIRED_ID, "expired");
    assert_entry_status(&list, "/allow_entries", REVIEW_DUE_ID, "review_due");
    assert_entry_status(&list, "/allow_entries", STALE_ID, "stale");
    assert_entry_status(&list, "/allow_entries", DRIFT_ID, "location_drift");
    assert_entry_matches(&list, HEADROOM_ID, 2);
    assert_entry_status(&list, "/allow_entries", MISSING_EVIDENCE_ID, "matched");
    assert_entry_count(&list, MISSING_EVIDENCE_ID, "evidence_count", 0);
    assert_entry_count(&list, BROKEN_EVIDENCE_ID, "broken_evidence_references", 1);
    assert_entry_count(&list, WEAK_EVIDENCE_ID, "weak_evidence_references", 1);

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

    let (headroom_path, headroom_result) =
        run_report(&root, "explain-headroom", &["explain", HEADROOM_ID]);
    assert_status("explain headroom", &headroom_result, true);
    assert_quiet("explain headroom", &headroom_result);
    let headroom = assert_saved_json_artifact(
        &headroom_path,
        "explain headroom",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&headroom, HEADROOM_ID, "matched");
    assert_eq!(
        headroom
            .pointer("/allow_entry/occurrence_limit")
            .and_then(Value::as_u64),
        Some(3),
        "explain should expose the configured occurrence limit"
    );
    assert_eq!(
        headroom
            .pointer("/summary/current_matches")
            .and_then(Value::as_u64),
        Some(2),
        "explain should expose the current matched count"
    );

    let (missing_evidence_path, missing_evidence_result) = run_report(
        &root,
        "explain-missing-evidence",
        &["explain", MISSING_EVIDENCE_ID],
    );
    assert_status("explain missing evidence", &missing_evidence_result, true);
    assert_quiet("explain missing evidence", &missing_evidence_result);
    let missing_evidence = assert_saved_json_artifact(
        &missing_evidence_path,
        "explain missing evidence",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&missing_evidence, MISSING_EVIDENCE_ID, "matched");
    assert_eq!(
        missing_evidence
            .pointer("/evidence_references")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "explain should show that the entry has no evidence references"
    );

    for (allow_id, evidence_status) in [
        (BROKEN_EVIDENCE_ID, "local_file_missing"),
        (WEAK_EVIDENCE_ID, "unstructured"),
    ] {
        let (path, result) = run_report(
            &root,
            &format!("explain-{allow_id}"),
            &["explain", allow_id],
        );
        assert_status(&format!("explain {allow_id}"), &result, true);
        assert_quiet(&format!("explain {allow_id}"), &result);
        let explanation = assert_saved_json_artifact(
            &path,
            &format!("explain {allow_id}"),
            "cargo-allow.explain.v1",
            "explain",
        );
        assert_explain_evidence_status(&explanation, evidence_status);
    }

    for (allow_id, status) in [(STALE_ID, "stale"), (DRIFT_ID, "location_drift")] {
        let (path, result) = run_report(
            &root,
            &format!("explain-{allow_id}"),
            &["explain", allow_id],
        );
        assert_status(&format!("explain {allow_id}"), &result, true);
        assert_quiet(&format!("explain {allow_id}"), &result);
        let explanation = assert_saved_json_artifact(
            &path,
            &format!("explain {allow_id}"),
            "cargo-allow.explain.v1",
            "explain",
        );
        assert_explain_status(&explanation, allow_id, status);
    }

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
    assert_entry_status(&worklist, "/work_items", STALE_ID, "stale");
    assert_entry_status(&worklist, "/work_items", DRIFT_ID, "location_drift");
    assert_work_item_kind(&worklist, HEADROOM_ID, "occurrence_headroom");
    assert_work_item_message(&worklist, HEADROOM_ID, "1 remaining");
    assert_work_item_kind(&worklist, MISSING_EVIDENCE_ID, "missing_evidence");
    assert_work_item_kind(&worklist, BROKEN_EVIDENCE_ID, "broken_evidence_link");
    assert_work_item_kind(&worklist, WEAK_EVIDENCE_ID, "weak_evidence_reference");

    for (command, args, should_succeed) in [
        ("audit", AUDIT_ARGS, true),
        ("check", CHECK_NO_NEW_ARGS, false),
        ("diff", DIFF_ARGS, false),
    ] {
        let (path, result) = run_report(&root, command, args);
        assert_status(command, &result, should_succeed);
        assert_quiet(command, &result);
        let report = assert_saved_json_artifact(&path, command, "cargo-allow.report.v1", command);
        assert_entry_status(&report, "/outcomes", EXPIRED_ID, "expired");
        assert_entry_status(&report, "/outcomes", REVIEW_DUE_ID, "review_due");
        assert_entry_status(&report, "/outcomes", STALE_ID, "stale");
        assert_entry_status(&report, "/outcomes", DRIFT_ID, "location_drift");
        assert_report_advisory_count(&report, "occurrence_headroom", 1);
        assert_report_advisory_count(&report, "policy_missing_evidence", 1);
        assert_report_advisory_count(&report, "broken_evidence_links", 1);
        assert_report_advisory_count(&report, "weak_evidence_references", 1);
    }

    remove_temp_root(root);
}

#[test]
fn stale_is_blocking_only_in_strict_while_location_drift_is_advisory() {
    let root = create_fixture("stale-drift-mode", false);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn relocate(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write stale/drift source: {err}")));
    fs::write(root.join("policy/allow.toml"), stale_drift_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write stale/drift policy: {err}")));

    let no_new_output = root.join("target/cargo-allow/no-new.json");
    let no_new = run_command(&root, &["check", "--mode", "no-new"], &no_new_output);
    assert_status("stale/drift no-new", &no_new, true);
    assert_quiet("stale/drift no-new", &no_new);
    let no_new_report = assert_saved_json_artifact(
        &no_new_output,
        "stale/drift no-new",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        no_new_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "stale and location_drift are advisory in no-new mode"
    );
    assert_entry_status(&no_new_report, "/outcomes", STALE_ID, "stale");
    assert_entry_status(&no_new_report, "/outcomes", DRIFT_ID, "location_drift");

    let strict_output = root.join("target/cargo-allow/strict.json");
    let strict = run_command(&root, &["check", "--mode", "strict"], &strict_output);
    assert_status("stale/drift strict", &strict, false);
    assert_quiet("stale/drift strict", &strict);
    let strict_report = assert_saved_json_artifact(
        &strict_output,
        "stale/drift strict",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        strict_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "stale is blocking in strict mode"
    );
    assert_entry_status(&strict_report, "/outcomes", STALE_ID, "stale");
    assert_entry_status(&strict_report, "/outcomes", DRIFT_ID, "location_drift");

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
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\npub fn reload(value: Option<u8>) -> u8 { value.unwrap() }\npub fn relocate(value: Option<u8>) -> u8 { value.unwrap() }\npub fn reserve(value: Option<u8>) -> u8 { let first = value.unwrap(); first + value.unwrap() }\npub fn missing_evidence(value: Option<u8>) -> u8 { value.unwrap() }\npub fn broken_evidence(value: Option<u8>) -> u8 { value.unwrap() }\npub fn weak_evidence(value: Option<u8>) -> u8 { value.unwrap() }\n"
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
    git(
        &root,
        &["commit", "--no-gpg-sign", "-m", "lifecycle corpus fixture"],
    );
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
    let additional = if include_expired {
        format!(
            r#"
[[allow]]
id = "{STALE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one stale policy entry for lifecycle review."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "gone"
callee = "unwrap"

[[allow]]
id = "{DRIFT_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture moves one allow entry away from its recorded location."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "relocate"
callee = "unwrap"

[allow.last_seen]
line = 99
column = 1

[[allow]]
id = "{HEADROOM_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture reserves one additional occurrence for a later match."
evidence = ["test:lifecycle_corpus"]
occurrence_limit = 3
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "reserve"
callee = "unwrap"

[[allow]]
id = "{MISSING_EVIDENCE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one matched entry without evidence for lifecycle repair."
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "missing_evidence"
callee = "unwrap"

[[allow]]
id = "{BROKEN_EVIDENCE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one broken local evidence reference for lifecycle repair."
evidence = ["doc:docs/missing-evidence.md"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "broken_evidence"
callee = "unwrap"

[[allow]]
id = "{WEAK_EVIDENCE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one weak evidence reference for lifecycle repair."
evidence = ["spreadsheet:manual-review"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "weak_evidence"
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
{additional}
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

fn stale_drift_policy() -> String {
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

[[allow]]
id = "{STALE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one stale policy entry for lifecycle review."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "gone"
callee = "unwrap"

[[allow]]
id = "{DRIFT_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture moves one allow entry away from its recorded location."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "relocate"
callee = "unwrap"

[allow.last_seen]
line = 99
column = 1
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

fn assert_entry_matches(value: &Value, allow_id: &str, matches: u64) {
    let entries = value
        .pointer("/allow_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/allow_entries should be an array"));
    let entry = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{allow_id} missing from /allow_entries"))
        });
    assert_eq!(
        entry.get("matches").and_then(Value::as_u64),
        Some(matches),
        "{allow_id} current match count"
    );
}

fn assert_entry_count(value: &Value, allow_id: &str, field: &str, count: u64) {
    let entries = value
        .pointer("/allow_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/allow_entries should be an array"));
    let entry = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{allow_id} missing from /allow_entries"))
        });
    assert_eq!(
        entry.get(field).and_then(Value::as_u64),
        Some(count),
        "{allow_id} {field}"
    );
}

fn assert_explain_evidence_status(value: &Value, status: &str) {
    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(Value::as_str),
        Some(status),
        "explain evidence status"
    );
}

fn assert_work_item_kind(value: &Value, allow_id: &str, kind: &str) {
    let items = value
        .pointer("/work_items")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/work_items should be an array"));
    let item = items
        .iter()
        .find(|item| item.get("allow_id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| std::panic::panic_any(format!("{allow_id} missing from /work_items")));
    assert_eq!(
        item.get("kind").and_then(Value::as_str),
        Some(kind),
        "{allow_id} work item kind"
    );
}

fn assert_work_item_message(value: &Value, allow_id: &str, fragment: &str) {
    let items = value
        .pointer("/work_items")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/work_items should be an array"));
    let item = items
        .iter()
        .find(|item| item.get("allow_id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| std::panic::panic_any(format!("{allow_id} missing from /work_items")));
    let message = item
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any(format!("{allow_id} work item has no message")));
    assert!(
        message.contains(fragment),
        "{allow_id} work item message should contain {fragment:?}: {message}"
    );
}

fn assert_report_advisory_count(value: &Value, advisory: &str, count: u64) {
    assert_eq!(
        value
            .pointer(&format!("/trend/{advisory}"))
            .and_then(Value::as_u64),
        Some(count),
        "report trend should expose {advisory} count"
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
