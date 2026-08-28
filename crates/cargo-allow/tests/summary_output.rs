mod support;

use std::fs;

use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn summary_artifact_commands_are_quiet_when_outputs_are_files() {
    assert_quiet_add_summary_output();
    assert_quiet_propose_summary_output();
    assert_quiet_migrate_summary_output();
}

#[test]
fn json_summary_without_file_is_rejected_before_command_work() {
    let cases = [
        vec!["propose", "--summary-format", "json"],
        vec![
            "add",
            "--kind",
            "panic",
            "--owner",
            "core",
            "--reason",
            "fixture",
            "--summary-format",
            "json",
        ],
        vec![
            "migrate",
            "--from",
            "legacy.toml",
            "--summary-format",
            "json",
        ],
    ];

    for args in cases {
        let result = cargo_allow_command()
            .args(args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow: {err}")));
        assert!(!result.status.success(), "JSON without output must fail");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(
                "--summary-format json requires --summary-output <path> to keep machine-readable output separate"
            ),
            "unexpected stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stdout.is_empty(),
            "rejected machine-readable commands must not emit stdout"
        );
    }
}

fn assert_quiet_add_summary_output() {
    let root = temp_root("summary-add-output");
    write_source_fixture(&root);
    write_policy(&root, empty_policy());

    let policy_output = root.join("policy/allow.added.toml");
    let summary_output = root.join("target/cargo-allow/add-summary.json");
    let result = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--owner")
        .arg("core")
        .arg("--reason")
        .arg("fixture")
        .arg("--evidence")
        .arg("test:fixture")
        .arg("--write")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&summary_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow add: {err}")));

    assert_success_and_quiet("add", &result);
    assert_file_contains(
        &policy_output,
        "[[allow]]",
        "add should write the updated policy artifact",
    );
    assert_saved_json_artifact(&summary_output, "add", "cargo-allow.add.v1", "add");

    remove_temp_root(root);
}

fn assert_quiet_propose_summary_output() {
    let root = temp_root("summary-propose-output");
    write_source_fixture(&root);
    write_policy(&root, empty_policy());

    let policy_output = root.join("policy/allow.proposed.toml");
    let summary_output = root.join("target/cargo-allow/propose-summary.json");
    let result = cargo_allow_command()
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--write")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&summary_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow propose: {err}")));

    assert_success_and_quiet("propose", &result);
    assert_file_contains(
        &policy_output,
        "[[allow]]",
        "propose should write the proposed policy artifact",
    );
    assert_saved_json_artifact(
        &summary_output,
        "propose",
        "cargo-allow.propose.v1",
        "propose",
    );

    remove_temp_root(root);
}

fn assert_quiet_migrate_summary_output() {
    let root = temp_root("summary-migrate-output");
    let repo_policy = root.join("legacy-policy");
    fs::create_dir_all(&repo_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create legacy policy dir: {err}")));
    fs::write(
        repo_policy.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write process fixture: {err}")));
    fs::write(
        repo_policy.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write network fixture: {err}")));

    let policy_output = root.join("policy/allow.migrated.toml");
    let summary_output = root.join("target/cargo-allow/migrate-summary.json");
    let result = cargo_allow_command()
        .arg("migrate")
        .arg("--root")
        .arg(&root)
        .arg("--repo-policy")
        .arg(&repo_policy)
        .arg("--out")
        .arg(&policy_output)
        .arg("--summary-format")
        .arg("json")
        .arg("--summary-output")
        .arg(&summary_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow migrate: {err}")));

    assert_success_and_quiet("migrate", &result);
    assert_file_contains(
        &policy_output,
        "[[allow]]",
        "migrate should write the canonical policy artifact",
    );
    assert_saved_json_artifact(
        &summary_output,
        "migrate",
        "cargo-allow.migrate.v1",
        "migrate",
    );

    remove_temp_root(root);
}

fn assert_success_and_quiet(command: &str, result: &std::process::Output) {
    assert_status(command, result, true);
    assert_stdout_empty(
        command,
        result,
        "should not emit policy text to stdout when policy output is a file",
    );
    assert_stderr_empty(
        command,
        result,
        "should not emit summary text to stderr when summary output is a file",
    );
}

fn write_source_fixture(root: &std::path::Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
}

fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}

fn write_policy(root: &std::path::Path, policy: &str) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn empty_policy() -> &'static str {
    r#"policy = "cargo-allow"
"#
}

fn process_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-cargo-install-cargo-deny"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
network_reach = true
called_by = [".github/workflows/ci.yml"]
owner = "release/ci"
reason = "Installs cargo-deny in the deny job."
created = "2026-05-09"
review_after = "2026-09-09"
"#
}

fn network_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"
"#
}
