mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

/// Full adoption arc: init → audit → check(fail) → why → add → check(pass).
///
/// Exercises every command an operator runs on first use, in order, against a
/// single fixture repo. Verifies the lifecycle works end-to-end: a finding is
/// detected, diagnosed with `why`, receipted with `add`, and the gate passes.
///
/// This complements `e2e_lifecycle.rs` (which covers init→add→explain→prune)
/// by adding `audit` and `why` into a single chained test (#2796).
#[test]
fn full_adoption_arc_from_init_to_passing_check() {
    let root = temp_root("e2e-adoption-arc");
    write_source_fixture(&root);
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);

    // 1. init — create the policy
    let init = cargo_allow_command()
        .current_dir(&root)
        .arg("init")
        .arg("--strict")
        .output()
        .unwrap_or_else(|err| panic!("run init: {err}"));
    assert_status("init", &init, true);
    assert!(
        init.stdout.windows(7).any(|w| w == b"created"),
        "init should report created"
    );

    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial fixture with policy"]);

    // 2. audit — inventory exceptions (should find the unwrap as a finding)
    let audit_output = root.join("target/cargo-allow/audit.json");
    let audit = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&audit_output)
        .output()
        .unwrap_or_else(|err| panic!("run audit: {err}"));
    assert_status("audit", &audit, true);
    let audit_report =
        assert_saved_json_artifact(&audit_output, "audit", "cargo-allow.report.v1", "audit");
    assert_eq!(
        audit_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "audit should not fail (advisory mode)"
    );

    // 3. check --mode no-new — should FAIL (unreceipted panic finding)
    let check_fail = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| panic!("run check (fail): {err}"));
    assert_status("check (fail)", &check_fail, false);
    let fail_report = serde_json::from_slice::<Value>(&check_fail.stdout)
        .unwrap_or_else(|err| panic!("check fail JSON: {err}"));
    assert_eq!(
        fail_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "check should fail the no-new gate before add"
    );
    assert_eq!(
        fail_report.pointer("/trend/new").and_then(Value::as_u64),
        Some(1),
        "check should report 1 new finding"
    );

    // 4. why — diagnose the unreceipted finding
    let why_output = root.join("target/cargo-allow/why.json");
    let why = cargo_allow_command()
        .arg("why")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&why_output)
        .output()
        .unwrap_or_else(|err| panic!("run why: {err}"));
    assert_status("why", &why, true);
    assert_stdout_empty("why", &why, "--output should not emit JSON to stdout");
    assert_stderr_empty(
        "why",
        &why,
        "--output should not emit side-channel status to stderr",
    );
    let why_report = assert_saved_json_artifact(&why_output, "why", "cargo-allow.why.v1", "why");
    assert_eq!(
        why_report
            .pointer("/outcome/status")
            .and_then(Value::as_str),
        Some("new"),
        "why should report the finding as new/unreceipted"
    );

    // 5. add — receipt the finding
    let add = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--id")
        .arg("allow-arc-panic")
        .arg("--owner")
        .arg("core")
        .arg("--reason")
        .arg("Adoption arc fixture intentionally unwraps.")
        .arg("--evidence")
        .arg("test:e2e_adoption_arc")
        .arg("--update")
        .output()
        .unwrap_or_else(|err| panic!("run add: {err}"));
    assert_status("add", &add, true);
    assert_file_contains(
        &root.join("policy/allow.toml"),
        "allow-arc-panic",
        "add should persist the entry",
    );

    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-m", "receipt panic finding"]);

    // 6. check --mode no-new — should PASS now
    let check_pass = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| panic!("run check (pass): {err}"));
    assert_status("check (pass)", &check_pass, true);
    let pass_report = serde_json::from_slice::<Value>(&check_pass.stdout)
        .unwrap_or_else(|err| panic!("check pass JSON: {err}"));
    assert_eq!(
        pass_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "check should pass the no-new gate after add"
    );
    assert_eq!(
        pass_report.pointer("/trend/new").and_then(Value::as_u64),
        Some(0),
        "check should report 0 new findings after add"
    );

    remove_temp_root(root);
}

fn write_source_fixture(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write source fixture: {err}"));
}

fn git(root: &Path, args: &[&str]) {
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

fn assert_file_contains(path: &Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(
        contents.contains(needle),
        "{message}: expected `{needle}` in {path:?}:\n{contents}"
    );
}
