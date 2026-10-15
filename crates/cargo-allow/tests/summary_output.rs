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

/// #4477: human stdout `list` without `--limit` shows a bounded first page
/// plus an explicit more-entries footer; JSON keeps the full set.
#[test]
fn list_human_stdout_is_bounded_by_default_and_json_is_not() {
    let root = temp_root("list-bounded-default");
    write_source_fixture(&root);
    let mut policy = String::from("policy = \"cargo-allow\"\n");
    for index in 0..25 {
        policy.push_str(&format!(
            r#"
[[allow]]
id = "allow-{index:02}"
kind = "non_rust_file"
family = "configuration"
path = "docs/file-{index:02}.md"
owner = "core"
classification = "fixture"
reason = "bounded list fixture"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/file-{index:02}.md"
target_fingerprint = "md"
glob = "docs/file-{index:02}.md"
"#
        ));
    }
    write_policy(&root, &policy);

    let run_list = |extra: &[&str]| {
        let mut command = cargo_allow_command();
        command.arg("list").arg("--root").arg(&root);
        for arg in extra {
            command.arg(arg);
        }
        command
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run list: {err}")))
    };

    let human = run_list(&[]);
    let human_text = String::from_utf8_lossy(&human.stdout).into_owned();
    assert!(human.status.success(), "list should succeed: {human_text}");
    let shown = human_text.matches("- [").count();
    assert_eq!(shown, 20, "default page shows 20 entries: {human_text}");
    assert!(
        human_text.contains("... 5 more entries not shown; rerun with --limit 25"),
        "footer names the remainder and the fix: {human_text}"
    );

    let explicit = run_list(&["--limit", "25"]);
    let explicit_text = String::from_utf8_lossy(&explicit.stdout).into_owned();
    assert_eq!(
        explicit_text.matches("- [").count(),
        25,
        "explicit --limit is honoured: {explicit_text}"
    );
    assert!(
        !explicit_text.contains("more entries not shown"),
        "no footer when nothing is hidden: {explicit_text}"
    );

    // Human --output artifacts keep their exact bytes: no footer even when
    // an explicit --limit truncates the file.
    let human_output = root.join("list.txt");
    let human_file = run_list(&[
        "--limit",
        "10",
        "--output",
        human_output.to_str().unwrap_or("list.txt"),
    ]);
    assert!(human_file.status.success(), "human --output list succeeds");
    let human_file_text = fs::read_to_string(&human_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read list.txt: {err}")));
    assert!(
        !human_file_text.contains("more entries not shown"),
        "--output artifacts must not gain the stdout footer: {human_file_text}"
    );
    assert_eq!(human_file_text.matches("- [").count(), 10);

    // With --offset the footer counts entries after this page and its
    // advice does not pretend --limit alone (keeping the offset) shows all.
    let offset_page = run_list(&["--offset", "10", "--limit", "10"]);
    let offset_text = String::from_utf8_lossy(&offset_page.stdout).into_owned();
    assert!(
        offset_text.contains("... 5 more entries after this page not shown"),
        "offset footer counts the tail after the page: {offset_text}"
    );

    let json_output = root.join("list.json");
    let json = run_list(&[
        "--format",
        "json",
        "--output",
        json_output.to_str().unwrap_or("list.json"),
    ]);
    assert!(json.status.success(), "json list should succeed");
    let json_text = fs::read_to_string(&json_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read list json: {err}")));
    for index in 0..25 {
        assert!(
            json_text.contains(&format!("\"allow-{index:02}\"")),
            "json keeps entry allow-{index:02}"
        );
    }

    // An explicit --offset alone keeps its historical meaning (all
    // remaining rows), so no default bound and no footer apply.
    let offset_only = run_list(&["--offset", "10"]);
    let offset_only_text = String::from_utf8_lossy(&offset_only.stdout).into_owned();
    assert_eq!(
        offset_only_text.matches("- [").count(),
        15,
        "offset-only shows every remaining entry: {offset_only_text}"
    );
    assert!(
        !offset_only_text.contains("more entries"),
        "offset-only gains no footer: {offset_only_text}"
    );

    remove_temp_root(root);
}

/// #4477 slice 2: human stdout `worklist` without paging flags shows a
/// bounded first page plus a remainder footer; JSON keeps the full queue.
#[test]
fn worklist_human_stdout_is_bounded_by_default_and_json_is_not() {
    let root = temp_root("worklist-bounded-default");
    // Clean source: the queue is exactly the 25 stale-entry items, so the
    // remainder arithmetic is exact.
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn clean() -> u8 { 1 }\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write clean source: {err}")));
    let mut policy = String::from("policy = \"cargo-allow\"\n");
    for index in 0..25 {
        policy.push_str(&format!(
            r#"
[[allow]]
id = "allow-{index:02}"
kind = "non_rust_file"
family = "configuration"
path = "docs/file-{index:02}.md"
owner = "core"
classification = "fixture"
reason = "bounded worklist fixture"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/file-{index:02}.md"
target_fingerprint = "md"
glob = "docs/file-{index:02}.md"
"#
        ));
    }
    write_policy(&root, &policy);

    let run_worklist = |extra: &[&str]| {
        let mut command = cargo_allow_command();
        command.arg("worklist").arg("--root").arg(&root);
        for arg in extra {
            command.arg(arg);
        }
        command
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run worklist: {err}")))
    };

    let human = run_worklist(&[]);
    let human_text = String::from_utf8_lossy(&human.stdout).into_owned();
    assert!(
        human.status.success(),
        "worklist should succeed: {human_text}"
    );
    assert!(
        human_text.contains("more work items not shown; page with --limit/--offset"),
        "default page ends with the remainder footer: {human_text}"
    );
    let rendered_items = human_text
        .lines()
        .filter(|line| line.starts_with("work-"))
        .count();
    assert_eq!(
        rendered_items, 20,
        "default page renders exactly 20 items: {human_text}"
    );
    // The queue also carries a coverage item beyond the 25 stale entries,
    // so the exact remainder is checked against the JSON queue below
    // (footer remainder == JSON total - rendered page).
    assert!(
        human_text.contains("use --format json to see the full queue"),
        "footer names the honest full-queue path (human renders cap at 80): {human_text}"
    );
    assert!(
        !human_text.contains("rerun with --limit 25 to see all"),
        "footer must not promise a full human view past the 80-item cap: {human_text}"
    );

    let json_output = root.join("worklist.json");
    let json = run_worklist(&[
        "--format",
        "json",
        "--output",
        json_output.to_str().unwrap_or("worklist.json"),
    ]);
    assert!(json.status.success(), "json worklist should succeed");
    let json_text = fs::read_to_string(&json_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read worklist json: {err}")));
    for index in 0..25 {
        assert!(
            json_text.contains(&format!("\"allow-{index:02}\"")),
            "json keeps queue entry allow-{index:02}"
        );
    }
    let json_doc: serde_json::Value = serde_json::from_str(&json_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse worklist json: {err}")));
    let json_total = json_doc
        .pointer("/work_items")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    assert!(
        human_text.contains(&format!(
            "... {} more work items not shown",
            json_total - 20
        )),
        "footer remainder equals JSON total minus the page: {human_text}"
    );

    let human_output = root.join("worklist.txt");
    let human_file = run_worklist(&["--output", human_output.to_str().unwrap_or("worklist.txt")]);
    assert!(
        human_file.status.success(),
        "human --output worklist succeeds"
    );
    let human_file_text = fs::read_to_string(&human_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read worklist.txt: {err}")));
    assert!(
        !human_file_text.contains("more work items not shown"),
        "--output artifacts must not gain the stdout footer"
    );
    assert!(
        human_file_text.contains("allow-24"),
        "--output keeps the full queue through the last entry"
    );

    remove_temp_root(root);
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
