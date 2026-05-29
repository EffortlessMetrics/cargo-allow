mod support;

use std::fs;

use support::{
    assert_file_contains, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn audit_with_output_file_does_not_emit_human_status_to_stderr() {
    let root = temp_root("audit-output");
    fs::write(root.join("tracked.txt"), "tracked\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    assert_file_contains(
        &output,
        "\"schema_id\": \"cargo-allow.report.v1\"",
        "audit output should be a report artifact",
    );

    remove_temp_root(root);
}
