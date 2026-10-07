mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

/// `refresh --write` updates `last_seen` in the policy TOML
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

/// An independently authored byte envelope makes whole-policy rendering visible.
#[test]
fn refresh_write_preserves_every_byte_outside_selected_coordinates()
-> Result<(), Box<dyn std::error::Error>> {
    for strings in [false, true] {
        let root = temp_root("e2e-refresh-byte-preservation");
        write_drift_fixture(&root);
        let policy = root.join("policy/allow.toml");
        let before = byte_preservation_policy(strings, 99, 99, 1);
        fs::write(&policy, &before)?;
        let output_path = root.join("refresh-preserved.json");
        let run = || {
            cargo_allow_command()
                .arg("refresh")
                .arg("--root")
                .arg(&root)
                .arg("--config")
                .arg(&policy)
                .arg("--allow-id")
                .arg("allow-drift")
                .arg("--write")
                .arg("--format")
                .arg("json")
                .arg("--output")
                .arg(&output_path)
                .output()
        };
        let refresh = run()?;
        assert_status("refresh byte fixture", &refresh, true);
        assert_stdout_empty("refresh byte fixture", &refresh, "saved output");
        assert_stderr_empty("refresh byte fixture", &refresh, "saved output");

        // The fixture's source puts the unwrap method name at line 3, column 50.
        // Construct the expected bytes from separate fragments, not writer output.
        let expected = byte_preservation_policy(strings, 99, 3, 50);
        let actual = fs::read_to_string(&policy)?;
        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "only the selected last_seen line/column value spans may change"
        );

        let mut semantic_expected = allow_policy::parse_policy(&before)?;
        let selected = semantic_expected
            .allow
            .iter_mut()
            .find(|entry| entry.id == "allow-drift")
            .ok_or("selected entry missing from fixture")?;
        selected.last_seen = Some(allow_core::LastSeen {
            line: 3,
            column: 50,
        });
        assert_eq!(
            allow_policy::parse_policy(&actual)?,
            semantic_expected,
            "all other parsed policy fields must remain equivalent"
        );
        let mut raw_expected: toml::Value = toml::from_str(before.trim_start_matches('\u{feff}'))?;
        let selected = raw_expected
            .get_mut("allow")
            .and_then(toml::Value::as_array_mut)
            .and_then(|entries| {
                entries.iter_mut().find(|entry| {
                    entry.get("id").and_then(toml::Value::as_str) == Some("allow-drift")
                })
            })
            .ok_or("selected raw entry missing")?;
        for (table, key, value) in [("last_seen", "line", 3), ("last_seen", "column", 50)] {
            let target = selected
                .get_mut(table)
                .and_then(|table| table.get_mut(key))
                .ok_or("selected raw coordinate missing")?;
            *target = if strings {
                toml::Value::String(value.to_string())
            } else {
                toml::Value::Integer(value)
            };
        }
        let raw_actual: toml::Value = toml::from_str(actual.trim_start_matches('\u{feff}'))?;
        assert_eq!(
            raw_actual, raw_expected,
            "raw values and omitted defaults are preserved"
        );

        let report = assert_saved_json_artifact(
            &output_path,
            "refresh",
            "cargo-allow.refresh.v1",
            "refresh",
        );
        assert_eq!(
            report
                .pointer("/mutation_receipt/result")
                .and_then(Value::as_str),
            Some("written")
        );
        let saved_output = fs::read(&output_path)?;
        fs::File::options().write(true).open(&policy)?.set_times(
            fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000),
            ),
        )?;
        let modified = fs::metadata(&policy)?.modified()?;
        let second = run()?;
        assert_eq!(
            second.status.code(),
            Some(2),
            "Matched target remains a Usage refusal"
        );
        assert!(String::from_utf8_lossy(&second.stderr).contains("matched"));
        assert_eq!(fs::read(&policy)?, expected.as_bytes());
        assert_eq!(fs::metadata(&policy)?.modified()?, modified);
        assert_eq!(
            fs::read(&output_path)?,
            saved_output,
            "refusal must preserve existing output"
        );
        remove_temp_root(root);
    }
    Ok(())
}

fn byte_preservation_policy(strings: bool, hint: u32, line: u32, column: u32) -> String {
    let coordinate = |value| {
        if strings {
            format!("'{value}'")
        } else {
            format!("{value}")
        }
    };
    let prefix = concat!(
        "\u{feff}# historical envelope\r\n",
        "schema_version = '0.1'\r\npolicy = 'cargo-allow'\n",
        "owner = 'custom-maintainer' # retained owner\r\nstatus = 'advisory'\n",
        "\r\n[requirements]\r\nstale_entries_fail = false\n",
        "\n[[allow]] # unrelated legacy entry\r\n",
        "id = 'allow-unrelated'\nkind = 'panic'\r\nfamily = 'expect'\n",
        "path = 'src/elsewhere.rs'\r\nowner = 'another-team'\n",
        "classification = 'reviewed_exception'\r\nreason = 'Retain this quoting.'\n",
        "evidence = ['test:refresh_unrelated']\r\n",
        "created = '2019-01-01'\nreview_after = '2099-01-01'\r\n",
        "[allow.selector]\nast_kind = 'method_call'\ncallee = 'expect'\r\n",
        "line_hint = '77' # unrelated legacy hint\r\n",
        "\n# selected entry retains comments and defaults\n[[allow]]\r\n",
        "id = 'allow-drift'\nkind = 'panic'\r\nfamily = 'unwrap'\n",
        "path = 'src/lib.rs'\r\nowner = 'custom-core'\n",
        "classification = 'reviewed_exception'\r\n",
        "reason = 'Selected location only.'\n",
        "evidence = ['test:refresh_write']\r\n",
        "created = '2019-01-01'\nreview_after = '2099-01-01'\r\n",
        "[allow.selector]\nast_kind = 'method_call'\r\ncontainer = 'relocate'\n",
        "callee = 'unwrap'\r\nline_hint = "
    );
    format!(
        "{prefix}{} # selected legacy hint\n\r\n[allow.last_seen]\r\nline = {} # selected line\ncolumn = {} # selected column; EOF",
        coordinate(hint),
        coordinate(line),
        coordinate(column)
    )
}

#[test]
fn refresh_write_refuses_unsupported_source_forms_without_any_output_change()
-> Result<(), Box<dyn std::error::Error>> {
    let fields = "id='allow-drift',kind='panic',family='unwrap',path='src/lib.rs',owner='core',classification='reviewed_exception',reason='Fixture',evidence=['test:refresh_write'],review_after='2099-01-01'";
    let selector = "selector={ast_kind='method_call',container='relocate',callee='unwrap'}";
    let inline_array = format!(
        "policy='cargo-allow'\nallow=[{{{fields},{selector},last_seen={{line=99,column=1}}}}]"
    );
    let explicit_fields = fields.split(',').collect::<Vec<_>>().join("\n");
    let inline_last_seen = format!(
        "policy='cargo-allow'\n[[allow]]\n{explicit_fields}\n{selector}\nlast_seen={{line=99,column=1}}"
    );
    let multiline = format!(
        "policy='cargo-allow'\n[[allow]]\n{explicit_fields}\n{selector}\n[allow.last_seen]\nline='''99'''\ncolumn=1"
    );
    let escaped = format!(
        "policy='cargo-allow'\n[[allow]]\n{explicit_fields}\n{selector}\n[allow.last_seen]\nline=\"\\u0039\\u0039\"\ncolumn=1"
    );
    for before in [inline_array, inline_last_seen, multiline, escaped] {
        let root = temp_root("e2e-refresh-unsupported");
        write_drift_fixture(&root);
        let policy = root.join("policy/allow.toml");
        allow_policy::parse_policy(&before)?;
        fs::write(&policy, &before)?;
        fs::File::options().write(true).open(&policy)?.set_times(
            fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000),
            ),
        )?;
        let modified = fs::metadata(&policy)?.modified()?;
        let output = root.join("refresh.json");
        let summary = root.join("summary.json");
        fs::write(&output, b"retained output")?;
        fs::write(&summary, b"retained summary")?;
        let result = cargo_allow_command()
            .arg("--command-summary-output")
            .arg(&summary)
            .arg("refresh")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--allow-id")
            .arg("allow-drift")
            .arg("--write")
            .arg("--format")
            .arg("json")
            .arg("--output")
            .arg(&output)
            .output()?;
        assert_eq!(
            result.status.code(),
            Some(1),
            "unsupported source must refuse: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("E0003_INVALID_POLICY"));
        assert_eq!(fs::read(&policy)?, before.as_bytes());
        assert_eq!(fs::metadata(&policy)?.modified()?, modified);
        assert_eq!(fs::read(&output)?, b"retained output");
        assert_eq!(fs::read(&summary)?, b"retained summary");
        assert_stdout_empty("unsupported refresh", &result, "refusal has no stdout");
        remove_temp_root(root);
    }
    Ok(())
}
