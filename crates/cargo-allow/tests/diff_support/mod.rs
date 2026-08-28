use std::fs;
use std::path::Path;
use std::process::Command;

use allow_core::SimpleDate;
use serde_json::Value;

use crate::support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command,
};

pub fn assert_saved_json_diff_failure(root: &Path, output: &Path) -> Value {
    assert_saved_json_diff(root, output, false)
}

pub fn assert_saved_json_diff_success(root: &Path, output: &Path) -> Value {
    assert_saved_json_diff(root, output, true)
}

fn assert_saved_json_diff(root: &Path, output: &Path, should_succeed: bool) -> Value {
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
        .unwrap_or_else(|err| panic!("run cargo-allow diff: {err}"));

    assert_status("diff", &result, should_succeed);
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
    assert_saved_json_artifact(output, "diff", "cargo-allow.report.v1", "diff")
}

pub fn write_diff_fixture(root: &Path, base_policy: String, head_policy: String) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write source: {err}"));
    fs::write(root.join("policy/allow.toml"), base_policy)
        .unwrap_or_else(|err| panic!("write base policy: {err}"));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    fs::write(root.join("policy/allow.toml"), head_policy)
        .unwrap_or_else(|err| panic!("write head policy: {err}"));
}

pub fn policy_with_evidence(evidence: Option<&str>) -> String {
    let review_after = SimpleDate::today_utc_approx().add_days(30);
    let evidence = evidence
        .map(|evidence| format!("evidence = [\"{evidence}\"]\n"))
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
{evidence}created = "2026-05-29"
review_after = "{review_after}"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

pub fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git {args:?}: {err}"));
    if !output.status.success() {
        panic!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
