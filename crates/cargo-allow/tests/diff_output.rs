mod diff_support;
mod json_assertions;
mod support;

use std::fs;

use diff_support::{
    assert_saved_json_diff_failure, assert_saved_json_diff_success, git, policy_with_evidence,
    write_diff_fixture,
};
use json_assertions::{assert_json_str, assert_json_u64};
use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn diff_json_with_output_file_does_not_emit_human_posture_to_stderr() {
    let root = temp_root("diff-output");
    write_diff_fixture(
        &root,
        policy_with_scope("path = \"src/lib.rs\""),
        policy_with_scope("glob = \"src/**\""),
    );
    let output = root.join("diff.json");

    assert_saved_json_diff_failure(&root, &output);
    assert_file_contains(
        &output,
        "\"scope_broadened\"",
        "diff output should include scope broadening posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_evidence_removed_policy_weakening() {
    let root = temp_root("diff-evidence-removed");
    write_diff_fixture(
        &root,
        policy_with_evidence(Some("test:parser_invariant")),
        policy_with_evidence(None),
    );
    let output = root.join("diff.json");

    assert_saved_json_diff_failure(&root, &output);
    assert_file_contains(
        &output,
        "\"evidence_removed\"",
        "diff output should include evidence removal posture",
    );
    assert_file_contains(
        &output,
        "\"net_posture\": \"worse\"",
        "diff output should classify evidence removal as worse",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_invalid_local_evidence_added_policy_failure() {
    let root = temp_root("diff-invalid-local-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:../outside.md")),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff invalid local evidence addition net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff invalid local evidence addition failure count",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "invalid local evidence added",
        "diff output should explain invalid local evidence addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_missing_local_evidence_added_policy_failure() {
    let root = temp_root("diff-missing-local-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:docs/missing.md")),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff missing local evidence addition net posture",
    );
    assert_json_u64(
        &value,
        "/summary/broken_evidence_links",
        1,
        "diff missing local evidence addition broken evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff missing local evidence addition policy failure count",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "broken local evidence added",
        "diff output should explain missing local evidence addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_validates_added_evidence_at_head_revision() {
    let root = temp_root("diff-head-missing-local-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:docs/head-only-missing.md")),
    );
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "add missing evidence reference"]);
    git(&root, &["tag", "head-missing-evidence"]);
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(
        root.join("docs/head-only-missing.md"),
        "working tree only evidence",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write working-tree evidence: {err}")));
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD~1")
        .arg("--head")
        .arg("head-missing-evidence")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow diff: {err}")));

    assert_status("diff", &result, false);
    assert_stdout_empty(
        "diff",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "diff",
        &result,
        "--output should not emit human posture rows to stderr",
    );
    let value = assert_saved_json_artifact(&output, "diff", "cargo-allow.report.v1", "diff");
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "explicit head missing evidence net posture",
    );
    assert_json_u64(
        &value,
        "/summary/broken_evidence_links",
        1,
        "explicit head missing evidence should report broken evidence from the head revision",
    );
    assert_json_u64(
        &value,
        "/diff/summary/current_failures",
        1,
        "explicit head current failures should use the head revision, not working-tree evidence",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "broken local evidence added",
        "diff output should validate added local evidence against explicit head revision",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_does_not_parse_working_tree_policy() {
    let root = temp_root("diff-head-invalid-working-policy");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:policy/head-evidence.md")),
    );
    fs::write(root.join("policy/head-evidence.md"), "head evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write head evidence: {err}")));
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "add valid evidence reference"]);
    git(&root, &["tag", "head-valid-evidence"]);
    fs::write(root.join("policy/allow.toml"), "this is not valid toml = [")
        .unwrap_or_else(|err| std::panic::panic_any(format!("corrupt working policy: {err}")));
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD~1")
        .arg("--head")
        .arg("head-valid-evidence")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow diff: {err}")));

    assert_status("diff", &result, true);
    assert_stdout_empty(
        "diff",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "diff",
        &result,
        "--output should not emit human posture rows to stderr",
    );
    let value = assert_saved_json_artifact(&output, "diff", "cargo-allow.report.v1", "diff");
    assert_json_str(
        &value,
        "/diff/net_posture",
        "improved",
        "explicit head should ignore invalid working-tree policy",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "improvement");

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_weak_evidence_added_as_review_required() {
    let root = temp_root("diff-weak-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("spreadsheet:manual-review")),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_success(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "review-required",
        "diff weak evidence addition net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_review_items",
        1,
        "diff weak evidence addition review item count",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "review");
    assert_file_contains(
        &output,
        "weak evidence added",
        "diff output should explain weak evidence addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_valid_evidence_added_as_improvement() {
    let root = temp_root("diff-valid-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("test:parser_invariant")),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_success(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "improved",
        "diff valid evidence addition net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_improvements",
        1,
        "diff valid evidence addition improvement count",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "improvement");
    assert_file_contains(
        &output,
        "evidence added",
        "diff output should explain valid evidence addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_lifecycle_extension_as_review_required() {
    let root = temp_root("diff-lifecycle-extended");
    write_diff_fixture(
        &root,
        policy_with_lifecycle("2026-08-01", "2026-07-01"),
        policy_with_lifecycle("2026-12-01", "2026-10-01"),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_success(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "review-required",
        "diff lifecycle extension net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_review_items",
        2,
        "diff lifecycle extension review item count",
    );
    assert_file_contains(
        &output,
        "\"kind\": \"expiry_extended\"",
        "diff output should include expiry extension posture",
    );
    assert_file_contains(
        &output,
        "\"kind\": \"review_after_extended\"",
        "diff output should include review_after extension posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_occurrence_limit_loosened_as_worse() {
    let root = temp_root("diff-occurrence-limit-loosened");
    write_diff_fixture(
        &root,
        policy_with_occurrence_limit(1),
        policy_with_occurrence_limit(3),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff occurrence-limit loosening net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff occurrence-limit loosening failure count",
    );
    assert_file_contains(
        &output,
        "\"kind\": \"occurrence_limit_loosened\"",
        "diff output should include occurrence-limit loosening posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_requirement_loosened_policy_failure() {
    let root = temp_root("diff-requirement-loosened");
    write_diff_fixture(
        &root,
        policy_with_owner_required(true),
        policy_with_owner_required(false),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff requirement loosened net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff requirement loosened failure count",
    );
    assert_policy_change(
        &value,
        "requirement_loosened",
        "requirements.owner_required",
        "fail",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_workspace_ignored_added_policy_failure() {
    let root = temp_root("diff-workspace-ignored-added");
    write_diff_fixture(
        &root,
        policy_with_workspace_ignored(&["policy/**"]),
        policy_with_workspace_ignored(&["policy/**", "src/**"]),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff workspace ignored addition net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff workspace ignored addition failure count",
    );
    assert_policy_change(
        &value,
        "workspace_ignored_added",
        "workspace.ignored",
        "fail",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_policy_owner_removed_policy_failure() {
    let root = temp_root("diff-policy-owner-removed");
    write_diff_fixture(
        &root,
        policy_with_policy_owner(Some("core/policy")),
        policy_with_policy_owner(None),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff policy owner removal net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff policy owner removal failure count",
    );
    assert_policy_change(&value, "policy_owner_removed", "policy.owner", "fail");

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_removed_policy_when_explicit_head_has_no_policy() {
    let root = temp_root("diff-head-missing-policy");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_evidence(Some("test:diff_head_missing_policy")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base policy"]);
    git(&root, &["tag", "base-policy"]);
    fs::remove_file(root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove policy: {err}")));
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "remove policy"]);
    git(&root, &["tag", "head-no-policy"]);
    git(
        &root,
        &["checkout", "base-policy", "--", "policy/allow.toml"],
    );
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("base-policy")
        .arg("--head")
        .arg("head-no-policy")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow diff: {err}")));

    assert_status("diff", &result, false);
    assert_stdout_empty(
        "diff",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "diff",
        &result,
        "--output should not emit human posture rows to stderr",
    );
    let value = assert_saved_json_artifact(&output, "diff", "cargo-allow.report.v1", "diff");
    assert_policy_change(&value, "removed_allow", "allow-unwrap", "improvement");

    remove_temp_root(root);
}

#[test]
fn diff_json_scans_missing_base_policy_with_empty_policy_not_head_policy() {
    let root = temp_root("diff-base-findings-empty-policy");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base without policy"]);
    let head_policy = policy_with_workspace_ignored(&["policy/**", "src/**"]);
    fs::write(root.join("policy/allow.toml"), head_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write head policy: {err}")));
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);

    let finding_changes = value
        .pointer("/diff/finding_changes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff finding_changes should be an array"));
    assert!(
        finding_changes.iter().any(|change| {
            change.get("change").and_then(Value::as_str) == Some("removed")
                && change.get("kind").and_then(Value::as_str) == Some("panic")
                && change.get("path").and_then(Value::as_str) == Some("src/lib.rs")
        }),
        "base scan should not use head workspace.ignored to hide source findings: {finding_changes:?}"
    );

    remove_temp_root(root);
}

fn policy_with_scope(scope: &str) -> String {
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = ["policy/**"]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
{scope}
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn policy_with_lifecycle(expires: &str, review_after: &str) -> String {
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = ["policy/**"]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
evidence = ["test:diff_json_reports_lifecycle_extension_as_review_required"]
created = "2026-05-29"
expires = "{expires}"
review_after = "{review_after}"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn policy_with_occurrence_limit(occurrence_limit: u32) -> String {
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = ["policy/**"]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
evidence = ["test:diff_json_reports_occurrence_limit_loosened_as_worse"]
occurrence_limit = {occurrence_limit}
created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn policy_with_owner_required(owner_required: bool) -> String {
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = ["policy/**"]

[requirements]
owner_required = {owner_required}

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
evidence = ["test:diff_json_reports_requirement_loosened_policy_failure"]
created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn policy_with_workspace_ignored(ignored: &[&str]) -> String {
    let ignored = ignored
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = [{ignored}]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
evidence = ["test:diff_json_reports_workspace_ignored_added_policy_failure"]
created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn policy_with_policy_owner(owner: Option<&str>) -> String {
    let owner = owner
        .map(|owner| format!("owner = \"{owner}\"\n"))
        .unwrap_or_default();
    format!(
        r#"policy = "cargo-allow"
{owner}
[workspace]
ignored = ["policy/**"]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
evidence = ["test:diff_json_reports_policy_owner_removed_policy_failure"]
created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}

fn assert_policy_change(value: &Value, kind: &str, allow_id: &str, severity: &str) {
    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let matched = changes.iter().any(|change| {
        change.get("kind").and_then(Value::as_str) == Some(kind)
            && change.get("allow_id").and_then(Value::as_str) == Some(allow_id)
            && change.get("severity").and_then(Value::as_str) == Some(severity)
    });
    assert!(
        matched,
        "expected policy change kind={kind} allow_id={allow_id} severity={severity}; got {changes:?}"
    );
}
