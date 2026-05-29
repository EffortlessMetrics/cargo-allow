mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

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

fn assert_saved_json_diff_failure(root: &Path, output: &Path) {
    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(root)
        .arg("--base")
        .arg("HEAD")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(output)
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
    assert_saved_json_artifact(output, "diff", "cargo-allow.report.v1", "diff");
}

fn write_diff_fixture(root: &Path, base_policy: String, head_policy: String) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(root.join("policy/allow.toml"), base_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write base policy: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    fs::write(root.join("policy/allow.toml"), head_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write head policy: {err}")));
}

fn policy_with_scope(scope: &str) -> String {
    format!(
        r#"policy = "cargo-allow"

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

fn policy_with_evidence(evidence: Option<&str>) -> String {
    let evidence = evidence
        .map(|evidence| format!("evidence = [\"{evidence}\"]\n"))
        .unwrap_or_default();
    format!(
        r#"policy = "cargo-allow"

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

fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}
