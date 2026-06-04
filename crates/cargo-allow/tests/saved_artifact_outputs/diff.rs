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

#[test]
fn saved_diff_output_covers_scope_broadening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-scope-broadened");
    fixture.write_panic_source();
    write_policy_with_scope(&fixture, "path = \"src/lib.rs\"");
    commit_fixture_base(&fixture.root);
    write_policy_with_scope(&fixture, "glob = \"src/**\"");

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
        "diff scope broadening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/scope_broadened")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff scope broadening summary count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("scope_broadened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-scope")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected scope broadening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "scope broadening severity"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("effective"),
        "scope broadening field"
    );
    assert_eq!(
        change
            .pointer("/scope/before")
            .and_then(serde_json::Value::as_str),
        Some("src/lib.rs"),
        "scope broadening before detail"
    );
    assert_eq!(
        change
            .pointer("/scope/after")
            .and_then(serde_json::Value::as_str),
        Some("src/**"),
        "scope broadening after detail"
    );
}

#[test]
fn saved_diff_output_covers_selector_precision_decrease_details() {
    let fixture = SourceTreeFixture::new("saved-diff-selector-precision-decreased");
    fixture.write_panic_source();
    write_policy_with_selector_container(&fixture, true);
    commit_fixture_base(&fixture.root);
    write_policy_with_selector_container(&fixture, false);

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
        "diff selector precision decrease net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/selector_precision_decreased")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff selector precision decrease summary count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("selector_precision_decreased")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-selector-precision")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected selector precision decrease policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "selector precision decrease severity"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/before")
            .and_then(serde_json::Value::as_u64),
        Some(70),
        "selector precision before detail"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/after")
            .and_then(serde_json::Value::as_u64),
        Some(55),
        "selector precision after detail"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/removed_fields/0")
            .and_then(serde_json::Value::as_str),
        Some("container"),
        "selector precision removed field"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/added_fields")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "selector precision added fields"
    );
}

#[test]
fn saved_diff_output_covers_evidence_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-evidence-removed");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, Some("doc:docs/safety.md"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(&fixture, None);

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
        "diff evidence removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_removed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff evidence removal summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_removal_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff evidence removal failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("evidence_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-evidence")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected evidence removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "evidence removal severity"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "evidence removal field"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/safety.md"),
        "evidence removal raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/added")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "evidence removal added references"
    );
}

#[test]
fn saved_diff_output_covers_weak_evidence_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-weak-evidence-added");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(&fixture, Some("spreadsheet:manual-review"));

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
        Some("review-required"),
        "diff weak evidence addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak evidence addition review item count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak evidence addition generic evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/weak_evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak evidence addition weak evidence count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("evidence_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-evidence")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected weak evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "weak evidence addition severity"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "weak evidence addition field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("spreadsheet:manual-review"),
        "weak evidence addition raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "weak evidence addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_lifecycle_extension_details() {
    let fixture = SourceTreeFixture::new("saved-diff-lifecycle-extended");
    fixture.write_panic_source();
    write_policy_with_lifecycle(&fixture, "2026-08-29", "2026-07-29");
    commit_fixture_base(&fixture.root);
    write_policy_with_lifecycle(&fixture, "2026-12-29", "2026-10-29");

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
        Some("review-required"),
        "diff lifecycle extension net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "diff lifecycle extension review item count"
    );

    assert_lifecycle_change(
        &value,
        "expiry_extended",
        "expires",
        "2026-08-29",
        "2026-12-29",
    );
    assert_lifecycle_change(
        &value,
        "review_after_extended",
        "review_after",
        "2026-07-29",
        "2026-10-29",
    );
}

#[test]
fn saved_diff_output_covers_baseline_debt_normalization_details() {
    let fixture = SourceTreeFixture::new("saved-diff-baseline-debt-normalized");
    fixture.write_panic_source();
    write_policy_with_baseline_classification(&fixture, "baseline_debt");
    commit_fixture_base(&fixture.root);
    write_policy_with_baseline_classification(&fixture, "reviewed_fixture");

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
        "diff baseline-debt normalization net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff baseline-debt normalization failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("baseline_debt_normalized")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-baseline-classification")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected baseline-debt normalization policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "baseline-debt normalization severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("classification"),
        "baseline-debt normalization metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "baseline-debt normalization metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("reviewed_fixture"),
        "baseline-debt normalization metadata after"
    );
}

#[test]
fn saved_diff_output_covers_added_baseline_debt_details() {
    let fixture = SourceTreeFixture::new("saved-diff-baseline-debt-added");
    fixture.write_panic_source();
    fixture.write_minimal_policy();
    commit_fixture_base(&fixture.root);
    write_policy_with_baseline_classification(&fixture, "baseline_debt");

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
        "diff added baseline-debt net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff added baseline-debt failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("baseline_debt_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-baseline-classification")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected added baseline-debt policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "added baseline-debt severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("added generated baseline debt")),
        "added baseline-debt message should explain generated debt: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_baseline_debt_introduction_details() {
    let fixture = SourceTreeFixture::new("saved-diff-baseline-debt-introduced");
    fixture.write_panic_source();
    write_policy_with_baseline_classification(&fixture, "reviewed_fixture");
    commit_fixture_base(&fixture.root);
    write_policy_with_baseline_classification(&fixture, "baseline_debt");

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
        "diff baseline-debt introduction net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff baseline-debt introduction failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("baseline_debt_introduced")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-baseline-classification")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected baseline-debt introduction policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "baseline-debt introduction severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("classification"),
        "baseline-debt introduction metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("reviewed_fixture"),
        "baseline-debt introduction metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "baseline-debt introduction metadata after"
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

fn write_policy_with_lifecycle(fixture: &SourceTreeFixture, expires: &str, review_after: &str) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-lifecycle"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff lifecycle extension details covered."
evidence = ["test:saved_diff_output_covers_lifecycle_extension_details"]
created = "2026-05-29"
expires = "{expires}"
review_after = "{review_after}"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_baseline_classification(fixture: &SourceTreeFixture, classification: &str) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-baseline-classification"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "{classification}"
reason = "Generated by cargo-allow propose; requires human review."
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn assert_lifecycle_change(
    value: &serde_json::Value,
    kind: &str,
    field: &str,
    before: &str,
    after: &str,
) {
    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-lifecycle")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected lifecycle policy change kind={kind}; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "lifecycle extension severity for {kind}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/field")
            .and_then(serde_json::Value::as_str),
        Some(field),
        "lifecycle field for {kind}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/before")
            .and_then(serde_json::Value::as_str),
        Some(before),
        "lifecycle before detail for {kind}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/after")
            .and_then(serde_json::Value::as_str),
        Some(after),
        "lifecycle after detail for {kind}"
    );
}

fn write_policy_with_optional_evidence(fixture: &SourceTreeFixture, evidence: Option<&str>) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    if let Some(path) = evidence.and_then(|reference| reference.strip_prefix("doc:")) {
        let path = fixture.root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        }
        fs::write(path, "# Safety evidence\n\nFixture evidence artifact.\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence fixture: {err}")));
    }
    let evidence = evidence
        .map(|evidence| format!("evidence = [\"{evidence}\"]\n"))
        .unwrap_or_default();
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-evidence"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff evidence removal details covered."
{evidence}created = "2026-05-29"
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

fn write_policy_with_selector_container(fixture: &SourceTreeFixture, include_container: bool) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let container = if include_container {
        "container = \"load\"\n"
    } else {
        ""
    };
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-selector-precision"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff selector precision loss details covered."
evidence = ["test:saved_diff_output_covers_selector_precision_decrease_details"]
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
{container}callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_scope(fixture: &SourceTreeFixture, scope: &str) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-scope"
kind = "panic"
family = "unwrap"
{scope}
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff scope-broadening posture details covered."
evidence = ["test:saved_diff_output_covers_scope_broadening_details"]
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
