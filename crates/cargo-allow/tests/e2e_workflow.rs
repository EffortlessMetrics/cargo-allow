mod e2e_support;
mod json_assertions;
mod support;

use std::fs;

use e2e_support::{assert_success_and_quiet, write_file};
use json_assertions::{assert_json_str, assert_json_u64};
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn propose_list_and_check_round_trip_with_cargo_prefix() {
    let root = temp_root("e2e-round-trip");
    write_panic_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    let policy_output = root.join("policy/allow.toml");
    let propose_summary = root.join("target/cargo-allow/propose.json");
    let propose = cargo_allow_command()
        .arg("allow")
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&propose_summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow propose: {err}")));

    assert_success_and_quiet("propose", &propose);
    let proposed = assert_saved_json_artifact(
        &propose_summary,
        "propose",
        "cargo-allow.propose.v1",
        "propose",
    );
    assert_json_u64(
        &proposed,
        "/summary/findings_scanned",
        1,
        "propose scanned one panic finding",
    );
    assert_json_u64(
        &proposed,
        "/summary/baseline_debt_entries_proposed",
        1,
        "propose generated one baseline entry",
    );
    assert_file_contains(
        &policy_output,
        "classification = \"baseline_debt\"",
        "propose should write baseline-debt policy entries",
    );

    let list_output = root.join("target/cargo-allow/list.json");
    let list = cargo_allow_command()
        .arg("allow")
        .arg("list")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_output)
        .arg("--kind")
        .arg("panic")
        .arg("--status")
        .arg("baseline_debt")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&list_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow list: {err}")));

    assert_success_and_quiet("list", &list);
    let listed = assert_saved_json_artifact(&list_output, "list", "cargo-allow.list.v1", "list");
    assert_json_u64(
        &listed,
        "/summary/allow_entries",
        1,
        "list should return the generated baseline entry",
    );
    assert_json_str(
        &listed,
        "/allow_entries/0/status",
        "baseline_debt",
        "list should expose the baseline-debt status",
    );

    let check_output = root.join("target/cargo-allow/check.json");
    let check = cargo_allow_command()
        .arg("allow")
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_output)
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&check_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_success_and_quiet("check", &check);
    let checked =
        assert_saved_json_artifact(&check_output, "check", "cargo-allow.report.v1", "check");
    assert_json_str(&checked, "/status", "passed", "check should pass");
    assert_json_u64(&checked, "/summary/matched", 1, "check matched count");
    assert_json_u64(&checked, "/summary/new", 0, "check new count");

    remove_temp_root(root);
}

#[test]
fn check_reports_new_findings_after_policy_baseline_is_outdated() {
    let root = temp_root("e2e-new-finding");
    write_panic_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    let policy_output = root.join("policy/allow.toml");
    let propose_summary = root.join("target/cargo-allow/propose.json");
    let propose = cargo_allow_command()
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&propose_summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow propose: {err}")));
    assert_success_and_quiet("propose", &propose);

    write_panic_source(
        &root,
        concat!(
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
            "pub fn reload(value: Result<u8, ()>) -> u8 { value.unwrap() }\n",
        ),
    );

    let check_output = root.join("target/cargo-allow/check-new.json");
    let check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy_output)
        .arg("--kind")
        .arg("panic")
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
    assert_json_str(&checked, "/status", "failed", "check should fail");
    assert_json_u64(&checked, "/summary/matched", 1, "check matched count");
    assert_json_u64(&checked, "/summary/new", 1, "check new count");
    assert_json_str(
        &checked,
        "/outcomes/1/status",
        "new",
        "check should report the added panic finding as new",
    );

    remove_temp_root(root);
}

#[test]
fn check_uses_policy_default_mode_when_mode_is_omitted() {
    let root = temp_root("e2e-policy-default-mode");
    write_panic_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
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
default_mode = "audit"
ignored = ["target/**"]
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
"#,
    );

    let audit_default_output = root.join("target/cargo-allow/check-audit-default.json");
    let audit_default = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("panic")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&audit_default_output)
        .output()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("run cargo-allow check default mode: {err}"))
        });

    assert_success_and_quiet("check default audit mode", &audit_default);
    let audit_default_report = assert_saved_json_artifact(
        &audit_default_output,
        "check default audit mode",
        "cargo-allow.report.v1",
        "check",
    );
    assert_json_str(
        &audit_default_report,
        "/status",
        "passed",
        "audit default should not fail the gate",
    );
    assert_json_u64(
        &audit_default_report,
        "/summary/new",
        1,
        "audit default should still report the new finding",
    );

    let no_new_output = root.join("target/cargo-allow/check-no-new.json");
    let no_new = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&no_new_output)
        .output()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("run cargo-allow check explicit no-new mode: {err}"))
        });

    assert_status("check explicit no-new mode", &no_new, false);
    assert_stdout_empty(
        "check explicit no-new mode",
        &no_new,
        "--output should not emit failure report JSON to stdout",
    );
    assert_stderr_empty(
        "check explicit no-new mode",
        &no_new,
        "--output should not emit human failure status to stderr",
    );
    let no_new_report = assert_saved_json_artifact(
        &no_new_output,
        "check explicit no-new mode",
        "cargo-allow.report.v1",
        "check",
    );
    assert_json_str(
        &no_new_report,
        "/status",
        "failed",
        "explicit no-new should override the audit policy default",
    );
    assert_json_u64(
        &no_new_report,
        "/summary/new",
        1,
        "explicit no-new should report the same new finding",
    );

    remove_temp_root(root);
}

/// #4481: both `propose --write` failures must name their actual fix.
/// The already-exists refusal must not inherit the generic writable/parent
/// Next line, and the out-of-root refusal must not inherit init/--config.
#[test]
fn propose_write_failures_name_the_actual_fix() {
    let root = temp_root("e2e-propose-write-guidance");
    write_panic_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );
    let policy_output = root.join("policy/allow.toml");
    let run_propose = |extra: &[&str]| {
        let mut command = cargo_allow_command();
        command
            .arg("propose")
            .arg("--root")
            .arg(&root)
            .arg("--kind")
            .arg("panic");
        for arg in extra {
            command.arg(arg);
        }
        command
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run propose: {err}")))
    };

    // A fresh in-root target writes successfully (control).
    let write_arg = policy_output.to_str().unwrap_or("policy/allow.toml");
    let first = run_propose(&["--write", write_arg]);
    assert!(
        first.status.success(),
        "first propose --write should succeed: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    // Already exists without --force: specific Next, no generic advice.
    let second = run_propose(&["--write", write_arg]);
    let second_text = String::from_utf8_lossy(&second.stderr).into_owned();
    assert!(
        !second.status.success(),
        "rewrite without --force must fail"
    );
    assert!(
        second_text.contains("error[E0007_ARTIFACT]"),
        "exists failure keeps its code: {second_text}"
    );
    assert!(
        second_text.contains("Next: pass --force to overwrite"),
        "exists failure names --force: {second_text}"
    );
    assert!(
        !second_text.contains("check the output path is writable"),
        "exists failure must not blame writability: {second_text}"
    );

    // --force still overwrites (negative control).
    let forced = run_propose(&["--write", write_arg, "--force"]);
    assert!(
        forced.status.success(),
        "--force overwrite should succeed: {}",
        String::from_utf8_lossy(&forced.stderr)
    );

    // A directory target is not the exists case: it must not receive
    // --force advice (force cannot overwrite a directory); the apply
    // layer's own refusal stands.
    let dir_target = root.join("policy-dir-target");
    fs::create_dir_all(&dir_target)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create dir target: {err}")));
    let dir_arg = dir_target.to_str().unwrap_or("dir");
    let dir_run = run_propose(&["--write", dir_arg]);
    let dir_text = String::from_utf8_lossy(&dir_run.stderr).into_owned();
    assert!(!dir_run.status.success(), "directory write must fail");
    assert!(
        !dir_text.contains("Next: pass --force to overwrite"),
        "directory target must not receive --force advice: {dir_text}"
    );

    // A symlink-to-file target also keeps the apply layer's refusal: the
    // apply layer refuses symlink leaves on replace, so --force advice
    // would be unusable there too. Creation is best-effort (symlink
    // privilege varies by host); the directory case above always runs.
    let link_target = root.join("policy-link-target.toml");
    let link_created = {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(&policy_output, &link_target).is_ok()
        }
        #[cfg(not(windows))]
        {
            std::os::unix::fs::symlink(&policy_output, &link_target).is_ok()
        }
    };
    if link_created {
        let link_arg = link_target.to_str().unwrap_or("link");
        let link_run = run_propose(&["--write", link_arg]);
        let link_text = String::from_utf8_lossy(&link_run.stderr).into_owned();
        assert!(!link_run.status.success(), "symlink write must fail");
        assert!(
            !link_text.contains("Next: pass --force to overwrite"),
            "symlink target must not receive --force advice: {link_text}"
        );
    }

    // Out-of-root target: fail closed, nothing written, specific Next.
    let outside = root.join("../outside-propose.toml");
    let outside_arg = outside.to_str().unwrap_or("outside");
    let blocked = run_propose(&["--write", outside_arg]);
    let blocked_text = String::from_utf8_lossy(&blocked.stderr).into_owned();
    assert!(!blocked.status.success(), "out-of-root write must fail");
    assert!(
        blocked_text.contains("Next: pass a --write path inside"),
        "containment failure names the inside-root fix: {blocked_text}"
    );
    assert!(
        !blocked_text.contains("run `cargo-allow init`"),
        "containment failure must not suggest init: {blocked_text}"
    );
    assert!(
        !outside.exists(),
        "out-of-root write must not create the target"
    );

    remove_temp_root(root);
}

fn write_panic_source(root: &std::path::Path, contents: &str) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(root.join("src/lib.rs"), contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
}

fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}
