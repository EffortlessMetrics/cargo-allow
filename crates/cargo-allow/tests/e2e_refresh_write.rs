mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

/// `refresh --write` updates `last_seen` and `line_hint` in the policy TOML
/// when a finding has drifted from its recorded location. The existing
/// `lifecycle_corpus.rs` test only checks the JSON receipt — this test verifies
/// the actual rewritten TOML contents, which is the real point of the command.
#[test]
fn refresh_write_updates_last_seen_in_policy_toml() {
    let root = temp_root("e2e-refresh-write");
    write_drift_fixture(&root);

    let policy_path = root.join("policy/allow.toml");

    // Verify the initial fixture has the stale last_seen line = 99
    let initial_policy = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read initial policy: {err}")));
    assert!(
        initial_policy.contains("line = 99"),
        "fixture should start with last_seen line = 99:\n{initial_policy}"
    );

    let refresh_output = root.join("target/cargo-allow/refresh.json");
    let common_summary = root.join("common-summary.json");
    let refresh = cargo_allow_command()
        .arg("--command-summary-output")
        .arg(&common_summary)
        .arg("refresh")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_path)
        .arg("--allow-id")
        .arg("allow-drift")
        .arg("--write")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&refresh_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run refresh --write: {err}")));

    assert_status("refresh --write", &refresh, true);
    assert_stdout_empty(
        "refresh --write",
        &refresh,
        "--output should not emit to stdout",
    );
    assert_stderr_empty(
        "refresh --write",
        &refresh,
        "--output should not emit to stderr",
    );

    let common: Value = serde_json::from_str(
        &fs::read_to_string(&common_summary)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read common summary: {err}"))),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse common summary: {err}")));
    assert_eq!(
        common.get("schema_id").and_then(Value::as_str),
        Some("cargo-allow.core-command-summary.v1")
    );
    assert_eq!(
        common.pointer("/operation").and_then(Value::as_str),
        Some("refresh")
    );
    assert_eq!(
        common.pointer("/posture").and_then(Value::as_str),
        Some("satisfied")
    );
    assert_eq!(
        common
            .pointer("/operation_effects/write_paths/0")
            .and_then(Value::as_str),
        Some("policy/allow.toml")
    );
    assert_eq!(
        common.pointer("/next_proof/args/0").and_then(Value::as_str),
        Some("check")
    );

    let report = assert_saved_json_artifact(
        &refresh_output,
        "refresh",
        "cargo-allow.refresh.v1",
        "refresh",
    );

    // The JSON receipt should confirm the write
    assert_eq!(
        report
            .pointer("/mode/write_requested")
            .and_then(Value::as_bool),
        Some(true),
        "refresh should report write_requested = true"
    );
    assert_eq!(
        report
            .pointer("/mutation_receipt/result")
            .and_then(Value::as_str),
        Some("written"),
        "refresh receipt result should be 'written'"
    );
    assert_eq!(
        report
            .pointer("/summary/lifecycle_preserved")
            .and_then(Value::as_bool),
        Some(true),
        "lifecycle dates should be preserved"
    );

    // The actual policy TOML should now have the updated last_seen line
    let updated_policy = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read updated policy: {err}")));
    assert!(
        !updated_policy.contains("line = 99"),
        "stale last_seen line = 99 should be gone after refresh:\n{updated_policy}"
    );
    assert!(
        updated_policy.contains("line = 3"),
        "last_seen should be updated to the finding's actual line (3):\n{updated_policy}"
    );

    // Lifecycle dates should be preserved (not modified by refresh)
    assert!(
        updated_policy.contains("created = \"2019-01-01\""),
        "created date should be preserved:\n{updated_policy}"
    );
    assert!(
        updated_policy.contains("review_after = \"2099-01-01\""),
        "review_after date should be preserved:\n{updated_policy}"
    );

    remove_temp_root(root);
}

fn write_drift_fixture(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));

    // The finding (unwrap call) is on line 3, but the policy records
    // last_seen line = 99, creating a location_drift.
    fs::write(
        root.join("src/lib.rs"),
        "// line 1\n// line 2\npub fn relocate(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    let policy = r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-drift"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Fixture entry that drifts from its recorded location."
evidence = ["test:refresh_write"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "relocate"
callee = "unwrap"

[allow.last_seen]
line = 99
column = 1
"#;
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--no-gpg-sign", "-m", "drift fixture"]);
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
