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
fn core_command_summary_installed_diff_projection() {
    let root = temp_root("diff-summary-projection");
    write_diff_fixture(
        &root,
        policy_with_scope("path = \"src/lib.rs\""),
        policy_with_scope("glob = \"src/**\""),
    );
    let sidecar = root.join("diff-summary.json");
    let output = cargo_allow_command()
        .arg("--command-summary-output")
        .arg(&sidecar)
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD")
        .arg("--format")
        .arg("human")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run diff summary: {err}")));
    assert_status("diff summary", &output, false);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("Result: findings (blocking)"), "{text}");
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read diff summary: {err}"))),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse diff summary: {err}")));
    assert_eq!(summary["operation"], "diff");
    assert_eq!(summary["subject"]["base"], "HEAD");
    assert_eq!(summary["posture"], "blocking");
    remove_temp_root(root);
}

#[test]
fn diff_human_statuses_style_only_terminal_labels() {
    let root = temp_root("diff-color");
    write_diff_fixture(
        &root,
        policy_with_scope("path = \"src/lib.rs\""),
        policy_with_scope("glob = \"src/**\""),
    );

    let run = |color: &str, output: Option<&std::path::Path>| {
        let mut command = cargo_allow_command();
        command
            .current_dir(&root)
            .arg("diff")
            .arg("--root")
            .arg(&root)
            .arg("--base")
            .arg("HEAD")
            .arg("--format")
            .arg("human")
            .arg("--color")
            .arg(color);
        if let Some(output) = output {
            command.arg("--output").arg(output);
        }
        command
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run colored diff: {err}")))
    };

    let plain = run("never", None);
    assert_status("plain diff", &plain, false);
    assert!(!String::from_utf8_lossy(&plain.stdout).contains('\u{1b}'));

    let styled = run("always", None);
    assert_status("styled diff", &styled, false);
    let styled_text = String::from_utf8_lossy(&styled.stdout);
    assert!(styled_text.contains('\u{1b}'));
    assert!(
        !styled_text.contains("allow-unwrap\u{1b}"),
        "allow IDs must remain plain"
    );

    let output = root.join("diff.txt");
    let written = run("always", Some(&output));
    assert_status("written diff", &written, false);
    let written_text = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read written diff: {err}")));
    assert!(!written_text.contains('\u{1b}'));

    remove_temp_root(root);
}

#[test]
fn diff_json_with_output_file_does_not_emit_human_posture_to_stderr() {
    let root = temp_root("diff-output");
    write_diff_fixture(
        &root,
        policy_with_scope("path = \"src/lib.rs\""),
        policy_with_scope("glob = \"src/**\""),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_u64(
        &value,
        "/diff/summary/scope_broadened",
        1,
        "diff scope broadening summary count",
    );
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

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_file_contains(
        &output,
        "\"evidence_removed\"",
        "diff output should include evidence removal posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/evidence_removed",
        1,
        "diff evidence removal generic evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/evidence_removal_failures",
        1,
        "diff evidence removal failure count",
    );
    assert_file_contains(
        &output,
        "\"net_posture\": \"worse\"",
        "diff output should classify evidence removal as worse",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_local_evidence_removed_policy_weakening() {
    let root = temp_root("diff-local-evidence-removed");
    write_diff_fixture(
        &root,
        policy_with_evidence(Some("doc:docs/safety/parser-spans.md")),
        policy_with_evidence(None),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff local evidence removal net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/evidence_removed",
        1,
        "diff local evidence removal generic evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/evidence_removal_failures",
        1,
        "diff local evidence removal failure count",
    );
    assert_policy_change(&value, "evidence_removed", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "local evidence removed",
        "diff output should identify local evidence removal posture",
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
    assert_json_u64(
        &value,
        "/diff/summary/evidence_added",
        1,
        "diff invalid local evidence addition generic evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/broken_evidence_added",
        1,
        "diff invalid local evidence addition broken evidence count",
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
    assert_json_u64(
        &value,
        "/diff/summary/evidence_added",
        1,
        "diff missing local evidence addition generic evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/broken_evidence_added",
        1,
        "diff missing local evidence addition broken evidence count",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "local evidence added outside compared source-tree inventory",
        "diff output should explain missing local evidence addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_untracked_local_evidence_added_policy_failure_by_default() {
    let root = temp_root("diff-untracked-local-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:docs/untracked.md")),
    );
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "untracked evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write untracked evidence: {err}")));
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff untracked local evidence addition net posture",
    );
    assert_json_u64(
        &value,
        "/summary/broken_evidence_links",
        1,
        "default diff inventory should not treat untracked local evidence as present",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "untracked local evidence addition should fail by default",
    );
    assert_json_u64(
        &value,
        "/diff/summary/evidence_added",
        1,
        "diff untracked local evidence addition generic evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/broken_evidence_added",
        1,
        "diff untracked local evidence addition broken evidence count",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "local evidence added outside compared source-tree inventory",
        "diff output should explain untracked local evidence addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_missing_local_link_added_policy_failure() {
    let root = temp_root("diff-missing-local-link-added");
    write_diff_fixture(
        &root,
        policy_with_links(None),
        policy_with_links(Some("doc:docs/missing-link.md")),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff missing local link addition net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff missing local link addition policy failure count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/link_added",
        1,
        "diff missing local link addition generic link count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/broken_link_added",
        1,
        "diff missing local link addition broken link count",
    );
    assert_policy_change(&value, "link_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "local link added outside compared source-tree inventory",
        "diff output should explain missing local link addition posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_local_link_removed_policy_failure() {
    let root = temp_root("diff-local-link-removed");
    write_diff_fixture(
        &root,
        policy_with_links(Some("doc:docs/rationale.md")),
        policy_with_links(None),
    );
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff local link removal net posture",
    );
    assert_json_u64(
        &value,
        "/diff/summary/policy_failures",
        1,
        "diff local link removal policy failure count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/link_removed",
        1,
        "diff local link removal generic link count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/link_removal_failures",
        1,
        "diff local link removal failure count",
    );
    assert_policy_change(&value, "link_removed", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "local traceability link removed",
        "diff output should explain local link removal posture",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_reports_missing_retained_local_link_current_failure() {
    let root = temp_root("diff-missing-retained-local-link");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(root.join("docs/rationale.md"), "linked rationale")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write rationale: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_links(Some("doc:docs/rationale.md")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    fs::remove_file(root.join("docs/rationale.md"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove rationale: {err}")));
    let output = root.join("diff.json");

    let value = assert_saved_json_diff_failure(&root, &output);
    assert_json_str(
        &value,
        "/diff/net_posture",
        "worse",
        "diff missing retained local link net posture",
    );
    assert_json_u64(
        &value,
        "/summary/broken_evidence_links",
        1,
        "diff should count missing retained local links as broken local references",
    );
    assert_json_u64(
        &value,
        "/diff/summary/current_failures",
        1,
        "missing retained local links should affect current diff failures",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_include_untracked_accepts_untracked_local_evidence_added() {
    let root = temp_root("diff-include-untracked-local-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:policy/untracked-evidence.md")),
    );
    fs::write(
        root.join("policy/untracked-evidence.md"),
        "untracked evidence",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write untracked evidence: {err}")));
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD")
        .arg("--include-untracked")
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
        "include-untracked diff should accept existing untracked local evidence",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "improvement");

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
        "local evidence added outside compared source-tree inventory",
        "diff output should validate added local evidence against explicit head revision",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_counts_invalid_local_evidence_as_broken() {
    let root = temp_root("diff-head-invalid-local-evidence-added");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("doc:docs/../src/lib.rs")),
    );
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "add invalid evidence reference"]);
    git(&root, &["tag", "head-invalid-evidence"]);
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD~1")
        .arg("--head")
        .arg("head-invalid-evidence")
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
    assert_json_u64(
        &value,
        "/summary/broken_evidence_links",
        1,
        "explicit head invalid local evidence should count as broken evidence",
    );
    assert_json_u64(
        &value,
        "/diff/summary/current_failures",
        1,
        "explicit head invalid local evidence should affect current failures",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "fail");
    assert_file_contains(
        &output,
        "invalid local evidence added",
        "explicit head invalid local evidence should preserve invalid-path posture detail",
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
fn diff_json_with_explicit_head_finds_policy_path_in_revision_when_working_policy_missing() {
    let root = temp_root("diff-head-missing-working-policy");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("test:head_policy_path_is_revision_backed")),
    );
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "add traceability evidence"]);
    git(&root, &["tag", "head-with-policy"]);
    fs::remove_file(root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove working policy: {err}")));
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD~1")
        .arg("--head")
        .arg("head-with-policy")
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
        "explicit head should find the default policy path in compared revisions",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "improvement");

    let explicit_config_output = root.join("diff-explicit-config.json");
    let explicit_config_result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--base")
        .arg("HEAD~1")
        .arg("--head")
        .arg("head-with-policy")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&explicit_config_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow diff: {err}")));

    assert_status("diff", &explicit_config_result, true);
    assert_stdout_empty(
        "diff",
        &explicit_config_result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "diff",
        &explicit_config_result,
        "--output should not emit human posture rows to stderr",
    );
    let explicit_config_value = assert_saved_json_artifact(
        &explicit_config_output,
        "diff",
        "cargo-allow.report.v1",
        "diff",
    );
    assert_json_str(
        &explicit_config_value,
        "/diff/net_posture",
        "improved",
        "explicit relative --config should be read from compared revisions",
    );
    assert_policy_change(
        &explicit_config_value,
        "evidence_added",
        "allow-unwrap",
        "improvement",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_prefers_revision_policy_path_over_working_tree_default() {
    let root = temp_root("diff-head-revision-policy-path");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(root.join("allow.toml"), root_policy_with_evidence(None))
        .unwrap_or_else(|err| std::panic::panic_any(format!("write base root policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base root policy"]);
    git(&root, &["tag", "base-root-policy"]);
    fs::write(
        root.join("allow.toml"),
        root_policy_with_evidence(Some("test:revision_policy_path")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write head root policy: {err}")));
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "add evidence to root policy"]);
    git(&root, &["tag", "head-root-policy"]);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create stale policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_evidence(Some("test:working_tree_policy_should_not_be_used")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write working policy: {err}")));
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("base-root-policy")
        .arg("--head")
        .arg("head-root-policy")
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
        "explicit head should use the policy path discovered in compared revisions",
    );
    assert_policy_change(&value, "evidence_added", "allow-unwrap", "improvement");

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_prefers_head_policy_path_over_base_only_default() {
    let root = temp_root("diff-head-policy-path-moved");
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
        policy_with_evidence(Some("test:base_policy_path")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write base policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base policy path"]);
    git(&root, &["tag", "base-default-policy"]);
    fs::remove_file(root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove base policy: {err}")));
    fs::write(
        root.join("allow.toml"),
        root_policy_with_evidence(Some("test:head_policy_path")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write head root policy: {err}")));
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "move policy path"]);
    git(&root, &["tag", "head-root-policy"]);
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("base-default-policy")
        .arg("--head")
        .arg("head-root-policy")
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
    assert_json_u64(
        &value,
        "/summary/new",
        0,
        "explicit head should receipt findings from the head policy path",
    );
    assert_json_u64(
        &value,
        "/diff/summary/current_failures",
        0,
        "explicit head should not report current failures from a base-only policy path",
    );
    assert_policy_change(&value, "added_allow", "allow-unwrap", "review");

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_inventory_count_respects_head_ignored_scopes() {
    let root = temp_root("diff-head-inventory-ignored");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::create_dir_all(root.join("ignored"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create ignored dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(
        root.join("ignored/panic.rs"),
        "fn ignored(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write ignored source: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_workspace_ignored(&["policy/**"]),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write base policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base inventory"]);
    git(&root, &["tag", "base-inventory"]);
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_workspace_ignored(&["policy/**", "ignored/**"]),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write head policy: {err}")));
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "ignore fixture source"]);
    git(&root, &["tag", "head-inventory"]);
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("base-inventory")
        .arg("--head")
        .arg("head-inventory")
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
    assert_json_u64(
        &value,
        "/inventory/files_scanned",
        1,
        "explicit head inventory count should apply head workspace.ignored scopes",
    );

    remove_temp_root(root);
}

#[test]
fn diff_json_with_explicit_head_rejects_missing_explicit_config_path() {
    let root = temp_root("diff-head-missing-explicit-config");
    write_diff_fixture(
        &root,
        policy_with_evidence(None),
        policy_with_evidence(Some("test:head_policy_path_is_revision_backed")),
    );
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "add traceability evidence"]);
    git(&root, &["tag", "head-with-policy"]);
    let output = root.join("diff.json");

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("missing-policy.toml")
        .arg("--base")
        .arg("HEAD~1")
        .arg("--head")
        .arg("head-with-policy")
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
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("policy config missing-policy.toml not found in compared revisions"),
        "diff should fail closed on a missing explicit --config path: {stderr}"
    );
    assert!(
        !output.exists(),
        "diff should not write a misleading empty-policy report for a missing explicit config"
    );

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
    assert_json_u64(
        &value,
        "/diff/summary/evidence_added",
        1,
        "diff weak evidence addition generic evidence count",
    );
    assert_json_u64(
        &value,
        "/diff/summary/weak_evidence_added",
        1,
        "diff weak evidence addition weak evidence count",
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

fn policy_with_links(link: Option<&str>) -> String {
    let links = link
        .map(|link| format!("links = [\"{link}\"]\n"))
        .unwrap_or_default();
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
evidence = ["test:parser_invariant"]
{links}created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
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

fn root_policy_with_evidence(evidence: Option<&str>) -> String {
    let evidence = evidence
        .map(|evidence| format!("evidence = [\"{evidence}\"]\n"))
        .unwrap_or_default();
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = ["allow.toml", "policy/**"]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
{evidence}created = "2026-05-29"
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
