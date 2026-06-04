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
fn saved_diff_output_covers_occurrence_limit_tightening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-occurrence-limit-tightened");
    fixture.write_panic_source();
    write_policy_with_occurrence_limit(&fixture, 3);
    commit_fixture_base(&fixture.root);
    write_policy_with_occurrence_limit(&fixture, 1);

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
        Some("improved"),
        "diff occurrence-limit tightening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff occurrence-limit tightening improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("occurrence_limit_tightened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-occurrence-limit")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected occurrence-limit tightening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "occurrence-limit tightening severity"
    );
    assert_eq!(
        change
            .pointer("/occurrence_limit/before")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "occurrence-limit tightening before detail"
    );
    assert_eq!(
        change
            .pointer("/occurrence_limit/after")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "occurrence-limit tightening after detail"
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
fn saved_diff_output_covers_scope_narrowing_details() {
    let fixture = SourceTreeFixture::new("saved-diff-scope-narrowed");
    fixture.write_panic_source();
    write_policy_with_scope(&fixture, "glob = \"src/**\"");
    commit_fixture_base(&fixture.root);
    write_policy_with_scope(&fixture, "path = \"src/lib.rs\"");

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
        Some("improved"),
        "diff scope narrowing net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/scope_narrowed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff scope narrowing summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "diff scope narrowing improvement count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/selector_precision_increased")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff scope narrowing selector precision increase count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("scope_narrowed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-scope")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected scope narrowing policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "scope narrowing severity"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("effective"),
        "scope narrowing field"
    );
    assert_eq!(
        change
            .pointer("/scope/before")
            .and_then(serde_json::Value::as_str),
        Some("src/**"),
        "scope narrowing before detail"
    );
    assert_eq!(
        change
            .pointer("/scope/after")
            .and_then(serde_json::Value::as_str),
        Some("src/lib.rs"),
        "scope narrowing after detail"
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
fn saved_diff_output_covers_selector_precision_increase_details() {
    let fixture = SourceTreeFixture::new("saved-diff-selector-precision-increased");
    fixture.write_panic_source();
    write_policy_with_selector_container(&fixture, false);
    commit_fixture_base(&fixture.root);
    write_policy_with_selector_container(&fixture, true);

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
        Some("improved"),
        "diff selector precision increase net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/selector_precision_increased")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff selector precision increase summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff selector precision increase improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("selector_precision_increased")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-selector-precision")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected selector precision increase policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "selector precision increase severity"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/before")
            .and_then(serde_json::Value::as_u64),
        Some(55),
        "selector precision increase before detail"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/after")
            .and_then(serde_json::Value::as_u64),
        Some(70),
        "selector precision increase after detail"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/added_fields/0")
            .and_then(serde_json::Value::as_str),
        Some("container"),
        "selector precision added field"
    );
    assert_eq!(
        change
            .pointer("/selector_precision/removed_fields")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "selector precision removed fields should be empty"
    );
}

#[test]
fn saved_diff_output_covers_selector_identity_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-selector-changed");
    fixture.write_panic_source();
    write_policy_with_selector_receiver(&fixture, "value");
    commit_fixture_base(&fixture.root);
    write_policy_with_selector_receiver(&fixture, "val");

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
        "diff selector identity change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/selector_changed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff selector identity change summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff selector identity change review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("selector_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-selector-identity")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected selector identity policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "selector identity change severity"
    );
    assert_eq!(
        change
            .pointer("/selector_identity/changed_fields/0")
            .and_then(serde_json::Value::as_str),
        Some("receiver_fingerprint"),
        "selector identity changed field"
    );
    assert!(
        change.get("selector_precision").is_none(),
        "equal-precision selector identity change should omit precision detail: {change:?}"
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
fn saved_diff_output_covers_weak_evidence_removal_improvement_details() {
    let fixture = SourceTreeFixture::new("saved-diff-weak-evidence-removed-improved");
    fixture.write_panic_source();
    write_policy_with_evidence_references(
        &fixture,
        &[
            "legacy-policy:proc-cargo-install-cargo-deny",
            "binary:cargo",
        ],
    );
    commit_fixture_base(&fixture.root);
    write_policy_with_evidence_references(
        &fixture,
        &["legacy-policy:proc-cargo-install-cargo-deny"],
    );

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
        Some("improved"),
        "diff weak evidence removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak evidence removal improvement count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_removed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak evidence removal generic evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_removal_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak evidence removal improvement evidence count"
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
                "expected weak evidence removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "weak evidence removal severity"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "weak evidence removal field"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed/0")
            .and_then(serde_json::Value::as_str),
        Some("binary:cargo"),
        "weak evidence removal raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/added")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "weak evidence removal added references"
    );
}

#[test]
fn saved_diff_output_covers_traceability_link_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-traceability-link-changed");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &["adr:docs/adr/0001.md", "issue:123"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_traceability_links(&fixture, &["issue:123", "pr:456"]);

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
        "diff traceability link change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff traceability link removal failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff traceability link addition improvement count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff traceability link removal summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removal_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff traceability link removal failure summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff traceability link addition summary count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let removed = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected traceability link removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        removed.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "traceability link removal severity"
    );
    assert_eq!(
        removed
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "traceability link removal field"
    );
    assert_eq!(
        removed
            .pointer("/evidence/removed/0")
            .and_then(serde_json::Value::as_str),
        Some("adr:docs/adr/0001.md"),
        "traceability link removal raw reference"
    );
    assert_eq!(
        removed
            .pointer("/evidence/added")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "traceability link removal added references"
    );

    let added = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected traceability link addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        added.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "traceability link addition severity"
    );
    assert_eq!(
        added
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "traceability link addition field"
    );
    assert_eq!(
        added
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("pr:456"),
        "traceability link addition raw reference"
    );
    assert_eq!(
        added
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "traceability link addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_broken_traceability_link_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-broken-traceability-link-added");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &[]);
    commit_fixture_base(&fixture.root);
    write_policy_with_missing_traceability_links(&fixture, &["doc:docs/missing-link.md"]);

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
        "diff broken traceability link addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken traceability link addition failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken traceability link addition summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken traceability link addition broken-link count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken traceability link inventory count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let added = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected broken traceability link addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        added.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "broken traceability link addition severity"
    );
    assert!(
        added
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message
                .contains("local link added outside compared source-tree inventory")),
        "broken traceability link addition message should route inventory repair: {added:?}"
    );
    assert_eq!(
        added
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "broken traceability link addition field"
    );
    assert_eq!(
        added
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/missing-link.md"),
        "broken traceability link addition raw reference"
    );
    assert_eq!(
        added
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "broken traceability link addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_weak_traceability_link_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-weak-traceability-link-added");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &[]);
    commit_fixture_base(&fixture.root);
    write_policy_with_traceability_links(
        &fixture,
        &["manual review note", "spreadsheet:manual-review"],
    );

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
        "diff weak traceability link addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link addition review count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link addition summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/weak_link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link addition weak-link count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let added = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected weak traceability link addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        added.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "weak traceability link addition severity"
    );
    assert!(
        added
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("weak traceability link added")),
        "weak traceability link addition message should name weak link posture: {added:?}"
    );
    assert_eq!(
        added
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "weak traceability link addition field"
    );
    assert_eq!(
        added
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("manual review note"),
        "weak traceability link addition first raw reference"
    );
    assert_eq!(
        added
            .pointer("/evidence/added/1")
            .and_then(serde_json::Value::as_str),
        Some("spreadsheet:manual-review"),
        "weak traceability link addition second raw reference"
    );
    assert_eq!(
        added
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "weak traceability link addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_weak_traceability_link_removal_improvement_details() {
    let fixture = SourceTreeFixture::new("saved-diff-weak-traceability-link-removed-improved");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &["issue:123", "spreadsheet:manual-review"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_traceability_links(&fixture, &["issue:123"]);

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
        Some("improved"),
        "diff weak traceability link removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link removal improvement count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link removal summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removal_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link removal improvement summary count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let removed = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected weak traceability link removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        removed.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "weak traceability link removal severity"
    );
    assert!(
        removed
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("weak traceability link removed")),
        "weak traceability link removal message should name weak link posture: {removed:?}"
    );
    assert_eq!(
        removed
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "weak traceability link removal field"
    );
    assert_eq!(
        removed
            .pointer("/evidence/removed/0")
            .and_then(serde_json::Value::as_str),
        Some("spreadsheet:manual-review"),
        "weak traceability link removal raw reference"
    );
    assert_eq!(
        removed
            .pointer("/evidence/added")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "weak traceability link removal added references"
    );
}

#[test]
fn saved_diff_output_covers_weak_traceability_link_removal_review_details() {
    let fixture = SourceTreeFixture::new("saved-diff-weak-traceability-link-removed-review");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &["spreadsheet:manual-review"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_traceability_links(&fixture, &[]);

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
        "diff weak traceability link removal review net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link removal review count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link removal summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removal_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff weak traceability link removal review summary count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let removed = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected weak traceability link removal review policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        removed.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "weak traceability link removal review severity"
    );
    assert!(
        removed
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("traceability link removed")),
        "weak traceability link removal review message should name removed link posture: {removed:?}"
    );
    assert_eq!(
        removed
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "weak traceability link removal review field"
    );
    assert_eq!(
        removed
            .pointer("/evidence/removed/0")
            .and_then(serde_json::Value::as_str),
        Some("spreadsheet:manual-review"),
        "weak traceability link removal review raw reference"
    );
    assert_eq!(
        removed
            .pointer("/evidence/added")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "weak traceability link removal review added references"
    );
}

#[test]
fn saved_diff_output_covers_typed_traceability_link_removal_review_details() {
    let fixture = SourceTreeFixture::new("saved-diff-typed-traceability-link-removed-review");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &["issue:123", "pr:456"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_traceability_links(&fixture, &["pr:456"]);

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
        "diff typed traceability link removal review net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff typed traceability link removal review count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff typed traceability link removal summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_removal_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff typed traceability link removal review summary count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let removed = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("link_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-links")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected typed traceability link removal review policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        removed.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "typed traceability link removal review severity"
    );
    let message = removed
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("typed traceability link removal message"));
    assert!(
        message.contains("traceability link removed"),
        "typed traceability link removal message should name removed link posture: {removed:?}"
    );
    assert!(
        !message.contains("local traceability link removed"),
        "typed traceability link removal should not be reported as local removal: {removed:?}"
    );
    assert_eq!(
        removed
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "typed traceability link removal review field"
    );
    assert_eq!(
        removed
            .pointer("/evidence/removed/0")
            .and_then(serde_json::Value::as_str),
        Some("issue:123"),
        "typed traceability link removal review raw reference"
    );
    assert_eq!(
        removed
            .pointer("/evidence/added")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "typed traceability link removal review added references"
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

#[test]
fn saved_diff_output_covers_policy_owner_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-policy-owner-removed");
    fixture.write_panic_source();
    write_policy_with_optional_policy_owner(&fixture, Some("core/policy"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_policy_owner(&fixture, None);

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
        "diff policy owner removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff policy owner removal failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("policy_owner_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("policy.owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected policy owner removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "policy owner removal severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("policy.owner removed")),
        "policy owner removal message should name the policy owner field: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "policy owner removal metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("core/policy"),
        "policy owner removal metadata before"
    );
    assert!(
        change
            .pointer("/metadata/after")
            .is_some_and(|value| value.is_null()),
        "policy owner removal metadata after should be null: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_requirement_loosening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-requirement-loosened");
    fixture.write_panic_source();
    write_policy_with_owner_required(&fixture, true);
    commit_fixture_base(&fixture.root);
    write_policy_with_owner_required(&fixture, false);

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
        "diff requirement loosening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff requirement loosening failure count"
    );
    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("requirement_loosened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("requirements.owner_required")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected requirement loosening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "requirement loosening severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("requirements.owner_required loosened")),
        "requirement loosening message should name the weakened requirement: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/requirement/field")
            .and_then(serde_json::Value::as_str),
        Some("owner_required"),
        "requirement loosening field"
    );
    assert_eq!(
        change
            .pointer("/requirement/before")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "requirement loosening before"
    );
    assert_eq!(
        change
            .pointer("/requirement/after")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "requirement loosening after"
    );
}

#[test]
fn saved_diff_output_covers_owner_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-owner-removed");
    fixture.write_panic_source();
    write_policy_with_optional_owner(&fixture, Some("core/tests"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_owner(&fixture, None);

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
        "diff owner removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff owner removal failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("owner_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected owner removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "owner removal severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "owner removal metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("core/tests"),
        "owner removal metadata before"
    );
    assert!(
        change
            .pointer("/metadata/after")
            .is_some_and(|value| value.is_null()),
        "owner removal metadata after should be null: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_owner_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-owner-changed");
    fixture.write_panic_source();
    write_policy_with_optional_owner(&fixture, Some("core/tests"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_owner(&fixture, Some("security/review"));

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
        "diff owner change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff owner change review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("owner_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected owner change policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "owner change severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "owner change metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("core/tests"),
        "owner change metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("security/review"),
        "owner change metadata after"
    );
}

#[test]
fn saved_diff_output_covers_owner_unassigned_details() {
    let fixture = SourceTreeFixture::new("saved-diff-owner-unassigned");
    fixture.write_panic_source();
    write_policy_with_optional_owner(&fixture, Some("core/tests"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_owner(&fixture, Some("unowned"));

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
        "diff owner-unassigned net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff owner-unassigned failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("owner_unassigned")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected owner-unassigned policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "owner-unassigned severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "owner-unassigned metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("core/tests"),
        "owner-unassigned metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("unowned"),
        "owner-unassigned metadata after"
    );
    assert!(
        changes.iter().all(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) != Some("owner_changed")
        }),
        "owner-unassigned should not be downgraded to owner_changed: {changes:?}"
    );
}

#[test]
fn saved_diff_output_covers_reason_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-reason-removed");
    fixture.write_panic_source();
    write_policy_with_optional_reason(
        &fixture,
        Some("Fixture keeps saved diff reason-removal posture details covered."),
    );
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_reason(&fixture, None);

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
        "diff reason removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff reason removal failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("reason_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-reason")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected reason removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "reason removal severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("reason"),
        "reason removal metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("Fixture keeps saved diff reason-removal posture details covered."),
        "reason removal metadata before"
    );
    assert!(
        change
            .pointer("/metadata/after")
            .is_some_and(|value| value.is_null()),
        "reason removal metadata after should be null: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_reason_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-reason-changed");
    fixture.write_panic_source();
    write_policy_with_optional_reason(
        &fixture,
        Some("Fixture keeps saved diff reason-change posture details covered."),
    );
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_reason(
        &fixture,
        Some("Fixture changed the retained exception rationale for review."),
    );

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
        "diff reason change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff reason change review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("reason_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-reason")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected reason change policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "reason change severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("reason"),
        "reason change metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("Fixture keeps saved diff reason-change posture details covered."),
        "reason change metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("Fixture changed the retained exception rationale for review."),
        "reason change metadata after"
    );
}

#[test]
fn saved_diff_output_covers_classification_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-classification-removed");
    fixture.write_panic_source();
    write_policy_with_optional_classification(&fixture, Some("reviewed_fixture"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_classification(&fixture, None);

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
        "diff classification removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff classification removal failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("classification_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-classification")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected classification removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "classification removal severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("classification"),
        "classification removal metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("reviewed_fixture"),
        "classification removal metadata before"
    );
    assert!(
        change
            .pointer("/metadata/after")
            .is_some_and(|value| value.is_null()),
        "classification removal metadata after should be null: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_classification_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-classification-changed");
    fixture.write_panic_source();
    write_policy_with_optional_classification(&fixture, Some("reviewed_fixture"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_classification(&fixture, Some("audited_fixture"));

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
        "diff classification change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff classification change review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("classification_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-classification")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected classification change policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "classification change severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("classification"),
        "classification change metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("reviewed_fixture"),
        "classification change metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("audited_fixture"),
        "classification change metadata after"
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

fn write_policy_with_optional_policy_owner(fixture: &SourceTreeFixture, owner: Option<&str>) {
    fixture.write_minimal_policy();
    let policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let owner = owner
        .map(|owner| format!("owner = \"{owner}\"\n"))
        .unwrap_or_default();
    let mut policy = policy.replacen("owner = \"core/policy\"\n", &owner, 1);
    policy.push_str(
        r#"

[[allow]]
id = "allow-policy-owner-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff policy-owner-removal posture details covered."
evidence = ["test:saved_diff_output_covers_policy_owner_removal_details"]
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_owner_required(fixture: &SourceTreeFixture, owner_required: bool) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy = policy.replace(
        "owner_required = true",
        &format!("owner_required = {owner_required}"),
    );
    policy.push_str(
        r#"

[[allow]]
id = "allow-requirement-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff requirement-loosening posture details covered."
evidence = ["test:saved_diff_output_covers_requirement_loosening_details"]
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_optional_owner(fixture: &SourceTreeFixture, owner: Option<&str>) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy = policy.replace("owner_required = true", "owner_required = false");
    let owner = owner
        .map(|owner| format!("owner = \"{owner}\"\n"))
        .unwrap_or_default();
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-owner"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
{owner}classification = "reviewed_fixture"
reason = "Fixture keeps saved diff owner-removal posture details covered."
evidence = ["test:saved_diff_output_covers_owner_removal_details"]
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

fn write_policy_with_optional_reason(fixture: &SourceTreeFixture, reason: Option<&str>) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy = policy.replace("reason_required = true", "reason_required = false");
    let reason = reason
        .map(|reason| format!("reason = \"{reason}\"\n"))
        .unwrap_or_default();
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-reason"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
{reason}evidence = ["test:saved_diff_output_covers_reason_removal_details"]
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

fn write_policy_with_optional_classification(
    fixture: &SourceTreeFixture,
    classification: Option<&str>,
) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy = policy.replace(
        "classification_required = true",
        "classification_required = false",
    );
    let classification = classification
        .map(|classification| format!("classification = \"{classification}\"\n"))
        .unwrap_or_default();
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-classification"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
{classification}reason = "Fixture keeps saved diff classification-removal posture details covered."
evidence = ["test:saved_diff_output_covers_classification_removal_details"]
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

fn write_policy_with_evidence_references(fixture: &SourceTreeFixture, evidence: &[&str]) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    for path in evidence
        .iter()
        .filter_map(|reference| reference.strip_prefix("doc:"))
    {
        let path = fixture.root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        }
        fs::write(path, "# Safety evidence\n\nFixture evidence artifact.\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence fixture: {err}")));
    }
    let evidence = if evidence.is_empty() {
        String::new()
    } else {
        format!(
            "evidence = [{}]\n",
            evidence
                .iter()
                .map(|reference| format!("\"{reference}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-evidence"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff weak evidence removal details covered."
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

fn write_policy_with_traceability_links(fixture: &SourceTreeFixture, links: &[&str]) {
    write_policy_with_traceability_links_inner(fixture, links, true);
}

fn write_policy_with_missing_traceability_links(fixture: &SourceTreeFixture, links: &[&str]) {
    write_policy_with_traceability_links_inner(fixture, links, false);
}

fn write_policy_with_traceability_links_inner(
    fixture: &SourceTreeFixture,
    links: &[&str],
    create_local_files: bool,
) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    if create_local_files {
        for path in links
            .iter()
            .copied()
            .filter_map(local_traceability_link_target)
        {
            let path = fixture.root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|err| {
                    std::panic::panic_any(format!("create traceability link dir: {err}"))
                });
            }
            fs::write(
                path,
                "# Traceability link\n\nFixture traceability artifact.\n",
            )
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("write traceability link fixture: {err}"))
            });
        }
    }
    let links = if links.is_empty() {
        String::new()
    } else {
        format!(
            "links = [{}]\n",
            links
                .iter()
                .map(|reference| format!("\"{reference}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-links"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff traceability link posture details covered."
evidence = ["test:saved_diff_output_covers_traceability_link_change_details"]
{links}created = "2026-05-29"
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

fn local_traceability_link_target(reference: &str) -> Option<&str> {
    reference
        .strip_prefix("adr:")
        .or_else(|| reference.strip_prefix("doc:"))
        .or_else(|| reference.strip_prefix("spec:"))
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

fn write_policy_with_selector_receiver(fixture: &SourceTreeFixture, receiver: &str) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-selector-identity"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff selector identity change details covered."
evidence = ["test:saved_diff_output_covers_selector_identity_change_details"]
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
receiver_fingerprint = "{receiver}"
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
