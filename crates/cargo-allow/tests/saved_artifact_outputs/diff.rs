use super::*;
use std::fs;

#[test]
fn saved_diff_output_covers_clean_posture_report_contract() {
    let fixture = SourceTreeFixture::new("saved-diff-clean");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();
    commit_fixture_base(&fixture.root);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow(&[
        "diff",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--base",
        "HEAD",
        "--format",
        "json",
        "--output",
        path_arg(&diff),
    ]);

    let value = assert_source_syntax_artifact_with_inventory(
        &diff,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("unchanged"),
        "diff net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/new_findings")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "diff new findings"
    );
}

#[test]
fn saved_diff_output_covers_occurrence_limit_loosening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-occurrence-limit-loosened");
    fixture.write_panic_source();
    write_policy_with_occurrence_limit(&fixture, 1);
    commit_fixture_base(&fixture.root);
    write_policy_with_occurrence_limit(&fixture, 3);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow_expect_status(
        &[
            "diff",
            "--root",
            fixture.root_str(),
            "--config",
            "policy/allow.toml",
            "--base",
            "HEAD",
            "--format",
            "json",
            "--output",
            path_arg(&diff),
        ],
        false,
    );

    let value = assert_source_syntax_artifact_with_inventory(
        &diff,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("worse"),
        "diff occurrence-limit loosening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff occurrence-limit loosening failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("occurrence_limit_loosened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-occurrence-limit")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected occurrence-limit loosening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "occurrence-limit loosening severity"
    );
    assert_eq!(
        change
            .pointer("/occurrence_limit/before")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "occurrence-limit before detail"
    );
    assert_eq!(
        change
            .pointer("/occurrence_limit/after")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "occurrence-limit after detail"
    );
}

fn write_policy_with_occurrence_limit(fixture: &SourceTreeFixture, occurrence_limit: u32) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-occurrence-limit"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff occurrence-limit posture details covered."
evidence = ["test:saved_diff_output_covers_occurrence_limit_loosening_details"]
occurrence_limit = {occurrence_limit}
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}
