use super::*;
use std::fs;
use std::process::Command;

#[test]
fn saved_diff_output_covers_clean_posture_report_contract() {
    let fixture = SourceTreeFixture::new("saved-diff-clean");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();
    commit_fixture_base(&fixture.root);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");
    let receipt = artifact_dir.join("diff.receipt.json");

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
        "--receipt",
        path_arg(&receipt),
    ]);

    let value = assert_source_syntax_artifact_with_inventory(
        &diff,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    let receipt_value = assert_source_syntax_artifact_with_inventory(
        &receipt,
        allow_report::RECEIPT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    assert_eq!(
        receipt_value
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("passed"),
        "diff receipt status"
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
    let receipt = artifact_dir.join("diff.receipt.json");

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
            "--receipt",
            path_arg(&receipt),
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
    let receipt_value = assert_source_syntax_artifact_with_inventory(
        &receipt,
        allow_report::RECEIPT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    assert_eq!(
        receipt_value
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("failed"),
        "failed diff should write a failed receipt"
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
fn saved_diff_output_covers_scope_retarget_details() {
    let fixture = SourceTreeFixture::new("saved-diff-scope-changed");
    write_policy_with_scope(&fixture, "glob = \"src/parser/**\"");
    commit_fixture_base(&fixture.root);
    write_policy_with_scope(&fixture, "glob = \"src/runtime/**\"");

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
        "diff scope retarget net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/scope_changed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff scope retarget summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff scope retarget review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("scope_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-scope")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected scope retarget policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "scope retarget severity"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("glob"),
        "scope retarget field"
    );
    assert_eq!(
        change
            .pointer("/scope/before")
            .and_then(serde_json::Value::as_str),
        Some("src/parser/**"),
        "scope retarget before detail"
    );
    assert_eq!(
        change
            .pointer("/scope/after")
            .and_then(serde_json::Value::as_str),
        Some("src/runtime/**"),
        "scope retarget after detail"
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
    write_policy_with_selector_receiver(&fixture, "param:0");
    commit_fixture_base(&fixture.root);
    write_policy_with_selector_receiver(&fixture, "param");

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
        "diff selector identity change net posture (exact-match: receiver change is identity loss)"
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
fn saved_diff_output_covers_exception_kind_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-kind-changed");
    write_policy_with_exception_identity(&fixture, "panic", "unwrap");
    commit_fixture_base(&fixture.root);
    write_policy_with_exception_identity(&fixture, "unsafe", "unwrap");

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
        "diff exception kind change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff exception kind change failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("kind_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-exception-identity")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected exception kind policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "exception kind change severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("changed governed exception kind")),
        "exception kind change message should name governed kind movement: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/exception_identity/field")
            .and_then(serde_json::Value::as_str),
        Some("kind"),
        "exception kind identity field"
    );
    assert_eq!(
        change
            .pointer("/exception_identity/before")
            .and_then(serde_json::Value::as_str),
        Some("panic"),
        "exception kind identity before"
    );
    assert_eq!(
        change
            .pointer("/exception_identity/after")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "exception kind identity after"
    );
}

#[test]
fn saved_diff_output_covers_exception_family_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-family-changed");
    write_policy_with_exception_identity(&fixture, "panic", "unwrap");
    commit_fixture_base(&fixture.root);
    write_policy_with_exception_identity(&fixture, "panic", "panic_macro");

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
        "diff exception family change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff exception family change failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("family_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-exception-identity")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected exception family policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "exception family change severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("changed governed exception family")),
        "exception family change message should name governed family movement: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/exception_identity/field")
            .and_then(serde_json::Value::as_str),
        Some("family"),
        "exception family identity field"
    );
    assert_eq!(
        change
            .pointer("/exception_identity/before")
            .and_then(serde_json::Value::as_str),
        Some("unwrap"),
        "exception family identity before"
    );
    assert_eq!(
        change
            .pointer("/exception_identity/after")
            .and_then(serde_json::Value::as_str),
        Some("panic_macro"),
        "exception family identity after"
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
fn saved_diff_output_covers_valid_local_evidence_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-valid-local-evidence-added");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    write_diff_evidence_fixture_doc(&fixture, "docs/safety.md");
    append_evidence_doc_allow(&fixture, "docs/safety.md");
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(&fixture, Some("doc:docs/safety.md"));
    append_evidence_doc_allow(&fixture, "docs/safety.md");

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
        "diff valid local evidence addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff valid local evidence addition improvement count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff valid local evidence addition generic evidence count"
    );
    assert!(
        value
            .pointer("/diff/summary/broken_evidence_added")
            .is_none(),
        "valid local evidence addition should not emit a broken-evidence count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "valid local evidence addition should not affect current evidence health"
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
                "expected valid local evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "valid local evidence addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("evidence added")),
        "valid local evidence addition message should identify evidence addition: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "valid local evidence addition field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/safety.md"),
        "valid local evidence addition raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "valid local evidence addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_include_untracked_local_evidence_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-include-untracked-local-evidence-added");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(&fixture, Some("doc:policy/untracked-evidence.md"));

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
        "--include-untracked",
        "--format",
        "json",
        "--output",
        path_arg(&diff),
    ]);

    let value = assert_source_syntax_artifact_with_inventory(
        &diff,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        "filesystem_include_untracked",
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("improved"),
        "include-untracked local evidence addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "include-untracked local evidence addition improvement count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "include-untracked local evidence addition generic evidence count"
    );
    assert!(
        value
            .pointer("/diff/summary/broken_evidence_added")
            .is_none(),
        "include-untracked local evidence addition should not emit a broken-evidence count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "include-untracked local evidence addition should not affect current evidence health"
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
                "expected include-untracked evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "include-untracked local evidence addition severity"
    );
    assert!(
        !change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("invalid local evidence added")
                || message.contains("missing local evidence added")),
        "include-untracked local evidence addition should not route broken evidence repair: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "include-untracked local evidence addition field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:policy/untracked-evidence.md"),
        "include-untracked local evidence addition raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "include-untracked local evidence addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_explicit_head_missing_local_evidence_details() {
    let fixture = SourceTreeFixture::new("saved-diff-explicit-head-missing-evidence-added");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_missing_optional_evidence(&fixture, Some("doc:docs/head-only-missing.md"));
    git_for_saved_diff(&fixture.root, &["add", "."]);
    git_for_saved_diff(
        &fixture.root,
        &["commit", "-m", "add missing evidence reference"],
    );
    git_for_saved_diff(&fixture.root, &["tag", "saved-head-missing-evidence"]);
    write_diff_evidence_fixture_doc(&fixture, "docs/head-only-missing.md");

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
            "HEAD~1",
            "--head",
            "saved-head-missing-evidence",
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
        "explicit-head missing local evidence net posture"
    );
    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head missing local evidence should count broken evidence from the head revision"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head current failures should use the head revision, not working-tree evidence"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head missing local evidence policy failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head missing local evidence generic evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head missing local evidence broken evidence count"
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
                "expected explicit-head evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "explicit-head missing local evidence severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message
                .contains("local evidence added outside compared source-tree inventory")),
        "explicit-head missing local evidence should route revision inventory repair: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "explicit-head missing local evidence field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/head-only-missing.md"),
        "explicit-head missing local evidence raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "explicit-head missing local evidence removed references"
    );
}

#[test]
fn saved_diff_output_covers_explicit_head_ignores_invalid_working_policy() {
    let fixture = SourceTreeFixture::new("saved-diff-explicit-head-invalid-working-policy");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(&fixture, Some("doc:policy/head-evidence.md"));
    git_for_saved_diff(&fixture.root, &["add", "."]);
    git_for_saved_diff(
        &fixture.root,
        &["commit", "-m", "add valid evidence reference"],
    );
    git_for_saved_diff(&fixture.root, &["tag", "saved-head-valid-evidence"]);
    fs::write(
        fixture.root.join("policy/allow.toml"),
        "this is not valid toml = [",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("corrupt working policy: {err}")));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow(&[
        "diff",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--base",
        "HEAD~1",
        "--head",
        "saved-head-valid-evidence",
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
        "explicit-head should ignore invalid working-tree policy"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head valid evidence improvement count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "explicit-head valid evidence should not inherit working-tree policy diagnostics"
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
                "expected explicit-head evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "explicit-head valid evidence severity"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "explicit-head valid evidence field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:policy/head-evidence.md"),
        "explicit-head valid evidence raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "explicit-head valid evidence removed references"
    );
}

#[test]
fn saved_diff_output_covers_explicit_head_finds_policy_when_working_policy_missing() {
    let fixture = SourceTreeFixture::new("saved-diff-explicit-head-missing-working-policy");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(
        &fixture,
        Some("test:saved-head-policy-path-is-revision-backed"),
    );
    git_for_saved_diff(&fixture.root, &["add", "."]);
    git_for_saved_diff(
        &fixture.root,
        &["commit", "-m", "add traceability evidence"],
    );
    git_for_saved_diff(&fixture.root, &["tag", "saved-head-with-policy"]);
    fs::remove_file(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove working policy: {err}")));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow(&[
        "diff",
        "--root",
        fixture.root_str(),
        "--base",
        "HEAD~1",
        "--head",
        "saved-head-with-policy",
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
        "explicit-head should find the policy path in compared revisions"
    );
    assert_eq!(
        value
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "explicit-head revision policy should receipt current source findings"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "explicit-head missing working policy should not create current failures"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head revision policy improvement count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "explicit-head revision policy should not inherit missing working-tree policy diagnostics"
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
                "expected explicit-head revision evidence addition; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "explicit-head revision evidence severity"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "explicit-head revision evidence field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("test:saved-head-policy-path-is-revision-backed"),
        "explicit-head revision evidence raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "explicit-head revision evidence removed references"
    );
}

#[test]
fn saved_diff_output_covers_explicit_head_policy_path_move_details() {
    let fixture = SourceTreeFixture::new("saved-diff-explicit-head-policy-path-moved");
    fixture.write_panic_source();
    fs::write(
        fixture.root.join("policy/allow.toml"),
        default_policy_with_optional_evidence(Some("test:saved-base-policy-path")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write base default policy: {err}")));
    commit_fixture_base(&fixture.root);
    git_for_saved_diff(&fixture.root, &["tag", "saved-base-default-policy"]);
    fs::remove_file(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove base policy: {err}")));
    fs::write(
        fixture.root.join("allow.toml"),
        root_policy_with_optional_evidence(Some("test:saved-head-policy-path")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write head root policy: {err}")));
    git_for_saved_diff(&fixture.root, &["add", "-A"]);
    git_for_saved_diff(&fixture.root, &["commit", "-m", "move policy path"]);
    git_for_saved_diff(&fixture.root, &["tag", "saved-head-root-policy"]);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow_expect_status(
        &[
            "diff",
            "--root",
            fixture.root_str(),
            "--base",
            "saved-base-default-policy",
            "--head",
            "saved-head-root-policy",
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
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "explicit-head policy-path move should receipt source findings from the head policy path"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "explicit-head policy-path move should not inherit current failures from a base-only policy path"
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("worse"),
        "explicit-head policy-path move net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head policy-path move review count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("added_allow")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected explicit-head policy-path added allow; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "explicit-head policy-path added allow severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("added a new allow entry")),
        "explicit-head policy-path move should identify the head policy receipt: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_explicit_head_inventory_ignored_scopes() {
    let fixture = SourceTreeFixture::new("saved-diff-explicit-head-inventory-ignored");
    fixture.write_panic_source();
    fs::create_dir_all(fixture.root.join("ignored"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create ignored dir: {err}")));
    fs::write(
        fixture.root.join("ignored/panic.rs"),
        "pub fn ignored(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write ignored source: {err}")));
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**", "ignored/**"]);
    git_for_saved_diff(&fixture.root, &["add", "-A"]);
    git_for_saved_diff(
        &fixture.root,
        &["commit", "-m", "ignore fixture source in head"],
    );
    git_for_saved_diff(&fixture.root, &["tag", "saved-head-ignored-inventory"]);
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**"]);

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
            "HEAD~1",
            "--head",
            "saved-head-ignored-inventory",
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
            .pointer("/inventory/files_scanned")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head inventory count should apply head workspace.ignored scopes"
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("worse"),
        "explicit-head ignored-scope addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head ignored-scope addition failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("workspace_ignored_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("workspace.ignored")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected explicit-head ignored-scope policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "explicit-head ignored-scope severity"
    );
    assert_eq!(
        change
            .pointer("/scope/after")
            .and_then(serde_json::Value::as_str),
        Some("ignored/**"),
        "explicit-head ignored-scope added scope"
    );
}

#[test]
fn saved_diff_output_covers_redundant_segment_evidence_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-redundant-segment-evidence-added");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    write_diff_evidence_fixture_doc(&fixture, "docs/safety.md");
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_evidence(&fixture, Some("doc:docs/./safety.md"));

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
        "diff redundant-segment evidence addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment evidence addition failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment evidence addition generic evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment evidence addition broken evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment evidence addition inventory count"
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
                "expected redundant-segment evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "redundant-segment evidence addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("invalid local evidence added")),
        "redundant-segment evidence addition message should name invalid local evidence: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "redundant-segment evidence addition field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/./safety.md"),
        "redundant-segment evidence addition raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "redundant-segment evidence addition removed references"
    );
}

#[test]
fn saved_diff_output_covers_broken_evidence_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-broken-evidence-added");
    fixture.write_panic_source();
    write_policy_with_optional_evidence(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_missing_optional_evidence(&fixture, Some("doc:docs/missing-evidence.md"));

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
        "diff broken evidence addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken evidence addition failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken evidence addition generic evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken evidence addition broken evidence count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff broken evidence addition inventory count"
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
                "expected broken evidence addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "broken evidence addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message
                .contains("local evidence added outside compared source-tree inventory")),
        "broken evidence addition message should route inventory repair: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("evidence"),
        "broken evidence addition field"
    );
    assert_eq!(
        change
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/missing-evidence.md"),
        "broken evidence addition raw reference"
    );
    assert_eq!(
        change
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "broken evidence addition removed references"
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
fn saved_diff_output_covers_missing_retained_traceability_link_current_failure_details() {
    let fixture = SourceTreeFixture::new("saved-diff-missing-retained-traceability-link");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &["doc:docs/rationale.md"]);
    commit_fixture_base(&fixture.root);
    fs::remove_file(fixture.root.join("docs/rationale.md"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove traceability doc: {err}")));

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
        "diff missing retained traceability link net posture"
    );
    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "missing retained traceability link should count as broken evidence health"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "missing retained traceability link should affect current diff failures"
    );
    assert_eq!(
        value
            .pointer("/diff/policy_changes")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "retained traceability link failure should not be reported as a policy edit"
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
fn saved_diff_output_covers_redundant_segment_traceability_link_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-redundant-segment-traceability-link-added");
    fixture.write_panic_source();
    write_policy_with_traceability_links(&fixture, &[]);
    write_diff_traceability_fixture_doc(&fixture, "docs/safety.md");
    commit_fixture_base(&fixture.root);
    write_policy_with_traceability_links(&fixture, &["doc:docs/./safety.md"]);

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
        "diff redundant-segment traceability link addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment traceability link addition failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment traceability link addition summary count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_link_added")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment traceability link addition broken-link count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff redundant-segment traceability link inventory count"
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
                "expected redundant-segment traceability link addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        added.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "redundant-segment traceability link addition severity"
    );
    assert!(
        added
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("invalid traceability link added")),
        "redundant-segment traceability link addition message should name invalid local link: {added:?}"
    );
    assert_eq!(
        added
            .pointer("/evidence/field")
            .and_then(serde_json::Value::as_str),
        Some("links"),
        "redundant-segment traceability link addition field"
    );
    assert_eq!(
        added
            .pointer("/evidence/added/0")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/./safety.md"),
        "redundant-segment traceability link addition raw reference"
    );
    assert_eq!(
        added
            .pointer("/evidence/removed")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "redundant-segment traceability link addition removed references"
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
    // Both entries must stay live on their side of the diff, so the
    // lifecycle dates are computed relative to today instead of hardcoded
    // calendar days the test would eventually sail past.
    let base_expires = allow_core::SimpleDate::today_utc_approx()
        .add_days(45)
        .to_string();
    let base_review_after = allow_core::SimpleDate::today_utc_approx()
        .add_days(30)
        .to_string();
    let head_expires = allow_core::SimpleDate::today_utc_approx()
        .add_days(90)
        .to_string();
    let head_review_after = allow_core::SimpleDate::today_utc_approx()
        .add_days(60)
        .to_string();
    write_policy_with_lifecycle(&fixture, &base_expires, &base_review_after);
    commit_fixture_base(&fixture.root);
    write_policy_with_lifecycle(&fixture, &head_expires, &head_review_after);

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
        "review",
        "expires",
        &base_expires,
        &head_expires,
    );
    assert_lifecycle_change(
        &value,
        "review_after_extended",
        "review",
        "review_after",
        &base_review_after,
        &head_review_after,
    );
}

#[test]
fn saved_diff_output_covers_lifecycle_shortening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-lifecycle-shortened");
    fixture.write_panic_source();
    // The head entry must stay live for the expected-success diff, so the
    // shortened lifecycle dates are computed relative to today instead of
    // hardcoded calendar days the test would eventually sail past.
    let base_expires = allow_core::SimpleDate::today_utc_approx()
        .add_days(90)
        .to_string();
    let base_review_after = allow_core::SimpleDate::today_utc_approx()
        .add_days(60)
        .to_string();
    let head_expires = allow_core::SimpleDate::today_utc_approx()
        .add_days(45)
        .to_string();
    let head_review_after = allow_core::SimpleDate::today_utc_approx()
        .add_days(30)
        .to_string();
    write_policy_with_lifecycle(&fixture, &base_expires, &base_review_after);
    commit_fixture_base(&fixture.root);
    write_policy_with_lifecycle(&fixture, &head_expires, &head_review_after);

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
        "diff lifecycle shortening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "diff lifecycle shortening improvement count"
    );

    assert_lifecycle_change(
        &value,
        "expiry_shortened",
        "improvement",
        "expires",
        &base_expires,
        &head_expires,
    );
    assert_lifecycle_change(
        &value,
        "review_after_shortened",
        "improvement",
        "review_after",
        &base_review_after,
        &head_review_after,
    );
}

#[test]
fn saved_diff_output_covers_created_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-created-removed");
    fixture.write_panic_source();
    write_policy_with_optional_created(&fixture, Some("2026-05-29"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_created(&fixture, None);

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
        "diff created-removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff created-removal failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("created_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-lifecycle")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected created-removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "created-removal severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("created date removed")),
        "created-removal message should name provenance loss: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/field")
            .and_then(serde_json::Value::as_str),
        Some("created"),
        "created-removal lifecycle field"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/before")
            .and_then(serde_json::Value::as_str),
        Some("2026-05-29"),
        "created-removal lifecycle before"
    );
    assert!(
        change
            .pointer("/lifecycle/after")
            .is_some_and(|value| value.is_null()),
        "created-removal lifecycle after should be null: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_created_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-created-changed");
    fixture.write_panic_source();
    write_policy_with_optional_created(&fixture, Some("2026-05-29"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_created(&fixture, Some("2026-06-05"));

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
        "diff created-change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff created-change review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("created_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-lifecycle")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected created-change policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "created-change severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("created date changed")),
        "created-change message should name provenance movement: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/field")
            .and_then(serde_json::Value::as_str),
        Some("created"),
        "created-change lifecycle field"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/before")
            .and_then(serde_json::Value::as_str),
        Some("2026-05-29"),
        "created-change lifecycle before"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/after")
            .and_then(serde_json::Value::as_str),
        Some("2026-06-05"),
        "created-change lifecycle after"
    );
}

#[test]
fn saved_diff_output_covers_created_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-created-added");
    fixture.write_panic_source();
    write_policy_with_optional_created(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_created(&fixture, Some("2026-06-05"));

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
        "diff created-addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff created-addition improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("created_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-lifecycle")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected created-addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "created-addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("created date added")),
        "created-addition message should name provenance addition: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/field")
            .and_then(serde_json::Value::as_str),
        Some("created"),
        "created-addition lifecycle field"
    );
    assert!(
        change
            .pointer("/lifecycle/before")
            .is_some_and(|value| value.is_null()),
        "created-addition lifecycle before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/lifecycle/after")
            .and_then(serde_json::Value::as_str),
        Some("2026-06-05"),
        "created-addition lifecycle after"
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
        true,
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
        Some("improved"),
        "diff baseline-debt normalization net posture (Improvement per #1926)"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "diff baseline-debt normalization failure count (0 per #1926)"
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
        Some("improvement"),
        "baseline-debt normalization severity (Improvement per #1926)"
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
fn saved_diff_output_covers_added_allow_details() {
    let fixture = SourceTreeFixture::new("saved-diff-allow-added");
    fixture.write_panic_source();
    fixture.write_minimal_policy();
    commit_fixture_base(&fixture.root);
    write_policy_with_reviewed_allow(&fixture);

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
        "diff added allow net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff added allow review count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("added_allow")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-added-review")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected added allow policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "added allow severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("added a new allow entry")),
        "added allow message should identify the new receipt: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_removed_allow_details() {
    let fixture = SourceTreeFixture::new("saved-diff-allow-removed");
    fixture.write_panic_source();
    write_policy_with_reviewed_allow(&fixture);
    commit_fixture_base(&fixture.root);
    fs::remove_file(fixture.root.join("src/lib.rs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove source fixture: {err}")));
    fixture.write_minimal_policy();

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
        "diff removed allow net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff removed allow improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("removed_allow")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-added-review")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected removed allow policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "removed allow severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("removed an allow entry")),
        "removed allow message should identify the removed receipt: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_explicit_head_removed_policy_details() {
    let fixture = SourceTreeFixture::new("saved-diff-explicit-head-policy-removed");
    fixture.write_panic_source();
    write_policy_with_reviewed_allow(&fixture);
    commit_fixture_base(&fixture.root);
    git_for_saved_diff(&fixture.root, &["tag", "saved-base-policy"]);
    fs::remove_file(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove policy fixture: {err}")));
    git_for_saved_diff(&fixture.root, &["add", "-A"]);
    git_for_saved_diff(&fixture.root, &["commit", "-m", "remove policy"]);
    git_for_saved_diff(&fixture.root, &["tag", "saved-head-no-policy"]);
    git_for_saved_diff(
        &fixture.root,
        &["checkout", "saved-base-policy", "--", "policy/allow.toml"],
    );

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
            "saved-base-policy",
            "--head",
            "saved-head-no-policy",
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
        "explicit-head removed policy net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "explicit-head removed policy current failure count"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "explicit-head removed policy improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("removed_allow")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-added-review")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected explicit-head removed policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "explicit-head removed policy severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("removed an allow entry")),
        "explicit-head removed policy message should identify the removed receipt: {change:?}"
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
fn saved_diff_output_covers_policy_owner_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-policy-owner-added");
    fixture.write_panic_source();
    write_policy_with_optional_policy_owner(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_policy_owner(&fixture, Some("core/policy"));

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
        "diff policy owner addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff policy owner addition improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("policy_owner_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("policy.owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected policy owner addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "policy owner addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("policy.owner added")),
        "policy owner addition message should name the policy owner field: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "policy owner addition metadata field"
    );
    assert!(
        change
            .pointer("/metadata/before")
            .is_some_and(|value| value.is_null()),
        "policy owner addition metadata before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("core/policy"),
        "policy owner addition metadata after"
    );
}

#[test]
fn saved_diff_output_covers_policy_owner_unassigned_details() {
    let fixture = SourceTreeFixture::new("saved-diff-policy-owner-unassigned");
    fixture.write_panic_source();
    write_policy_with_optional_policy_owner(&fixture, Some("core/policy"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_policy_owner(&fixture, Some("unowned"));

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
        "diff policy owner unassigned net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff policy owner unassigned failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("policy_owner_unassigned")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("policy.owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected policy owner unassigned policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "policy owner unassigned severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("policy.owner unassigned")),
        "policy owner unassigned message should name the policy owner field: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "policy owner unassigned metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("core/policy"),
        "policy owner unassigned metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("unowned"),
        "policy owner unassigned metadata after"
    );
}

#[test]
fn saved_diff_output_covers_policy_owner_change_details() {
    let fixture = SourceTreeFixture::new("saved-diff-policy-owner-changed");
    fixture.write_panic_source();
    write_policy_with_optional_policy_owner(&fixture, Some("core/policy"));
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_policy_owner(&fixture, Some("security/review"));

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
        "diff policy owner change net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff policy owner change review item count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("policy_owner_changed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("policy.owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected policy owner change policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "policy owner change severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("policy.owner changed")),
        "policy owner change message should name the policy owner field: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "policy owner change metadata field"
    );
    assert_eq!(
        change
            .pointer("/metadata/before")
            .and_then(serde_json::Value::as_str),
        Some("core/policy"),
        "policy owner change metadata before"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("security/review"),
        "policy owner change metadata after"
    );
}

#[test]
fn saved_diff_output_covers_policy_status_weakening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-policy-status-weakened");
    fixture.write_panic_source();
    write_policy_with_policy_status(&fixture, "active");
    commit_fixture_base(&fixture.root);
    write_policy_with_policy_status(&fixture, "advisory");

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
        "diff policy status weakening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff policy status weakening failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("policy_status_weakened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("policy.status")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected policy status weakening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "policy status weakening severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("policy.status weakened")),
        "policy status weakening message should name weakened status: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/policy_status/before")
            .and_then(serde_json::Value::as_str),
        Some("active"),
        "policy status weakening before"
    );
    assert_eq!(
        change
            .pointer("/policy_status/after")
            .and_then(serde_json::Value::as_str),
        Some("advisory"),
        "policy status weakening after"
    );
}

#[test]
fn saved_diff_output_covers_policy_status_tightening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-policy-status-tightened");
    fixture.write_panic_source();
    write_policy_with_policy_status(&fixture, "advisory");
    commit_fixture_base(&fixture.root);
    write_policy_with_policy_status(&fixture, "active");

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
        "diff policy status tightening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff policy status tightening improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("policy_status_tightened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("policy.status")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected policy status tightening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "policy status tightening severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("policy.status tightened")),
        "policy status tightening message should name tightened status: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/policy_status/before")
            .and_then(serde_json::Value::as_str),
        Some("advisory"),
        "policy status tightening before"
    );
    assert_eq!(
        change
            .pointer("/policy_status/after")
            .and_then(serde_json::Value::as_str),
        Some("active"),
        "policy status tightening after"
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
fn saved_diff_output_covers_requirement_tightening_details() {
    let fixture = SourceTreeFixture::new("saved-diff-requirement-tightened");
    fixture.write_panic_source();
    write_policy_with_owner_required(&fixture, false);
    commit_fixture_base(&fixture.root);
    write_policy_with_owner_required(&fixture, true);

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
        "diff requirement tightening net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff requirement tightening improvement count"
    );
    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("requirement_tightened")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("requirements.owner_required")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected requirement tightening policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "requirement tightening severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("requirements.owner_required tightened")),
        "requirement tightening message should name the strengthened requirement: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/requirement/field")
            .and_then(serde_json::Value::as_str),
        Some("owner_required"),
        "requirement tightening field"
    );
    assert_eq!(
        change
            .pointer("/requirement/before")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "requirement tightening before"
    );
    assert_eq!(
        change
            .pointer("/requirement/after")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "requirement tightening after"
    );
}

#[test]
fn saved_diff_output_covers_workspace_ignored_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-workspace-ignored-added");
    fixture.write_panic_source();
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**", "src/**"]);

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
        "diff workspace ignored addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff workspace ignored addition failure count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("workspace_ignored_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("workspace.ignored")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected workspace ignored addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("fail"),
        "workspace ignored addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("added ignored source-tree scope")),
        "workspace ignored addition message should name ignored source-tree scope: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("effective"),
        "workspace ignored addition scope field"
    );
    assert!(
        change
            .pointer("/scope/before")
            .is_some_and(|value| value.is_null()),
        "workspace ignored addition scope before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/scope/after")
            .and_then(serde_json::Value::as_str),
        Some("src/**"),
        "workspace ignored addition scope after"
    );
}

#[test]
fn saved_diff_output_covers_workspace_ignored_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-workspace-ignored-removed");
    fixture.write_panic_source();
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**", "ignored/**"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_workspace_ignored(&fixture, &["policy/**", "target/**"]);

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
        "diff workspace ignored removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff workspace ignored removal improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("workspace_ignored_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("workspace.ignored")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected workspace ignored removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "workspace ignored removal severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("removed ignored source-tree scope")),
        "workspace ignored removal message should name ignored source-tree scope: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("effective"),
        "workspace ignored removal scope field"
    );
    assert_eq!(
        change
            .pointer("/scope/before")
            .and_then(serde_json::Value::as_str),
        Some("ignored/**"),
        "workspace ignored removal scope before"
    );
    assert!(
        change
            .pointer("/scope/after")
            .is_some_and(|value| value.is_null()),
        "workspace ignored removal scope after should be null: {change:?}"
    );
}

#[test]
fn saved_diff_output_covers_workspace_generated_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-workspace-generated-added");
    fixture.write_panic_source();
    write_policy_with_workspace_generated(&fixture, &["target/**", "vendor/**"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_workspace_generated(&fixture, &["target/**", "vendor/**", "schemas/**"]);

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
        "diff workspace generated addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_review_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff workspace generated addition review count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("workspace_generated_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("workspace.generated")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected workspace generated addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("review"),
        "workspace generated addition severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("added generated source-tree scope")),
        "workspace generated addition message should name generated source-tree scope: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("effective"),
        "workspace generated addition scope field"
    );
    assert!(
        change
            .pointer("/scope/before")
            .is_some_and(|value| value.is_null()),
        "workspace generated addition scope before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/scope/after")
            .and_then(serde_json::Value::as_str),
        Some("schemas/**"),
        "workspace generated addition scope after"
    );
}

#[test]
fn saved_diff_output_covers_workspace_generated_removal_details() {
    let fixture = SourceTreeFixture::new("saved-diff-workspace-generated-removed");
    fixture.write_panic_source();
    write_policy_with_workspace_generated(&fixture, &["target/**", "vendor/**", "schemas/**"]);
    commit_fixture_base(&fixture.root);
    write_policy_with_workspace_generated(&fixture, &["target/**", "vendor/**"]);

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
        "diff workspace generated removal net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff workspace generated removal improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str)
                == Some("workspace_generated_removed")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("workspace.generated")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected workspace generated removal policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "workspace generated removal severity"
    );
    assert!(
        change
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("removed generated source-tree scope")),
        "workspace generated removal message should name generated source-tree scope: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/scope/field")
            .and_then(serde_json::Value::as_str),
        Some("effective"),
        "workspace generated removal scope field"
    );
    assert_eq!(
        change
            .pointer("/scope/before")
            .and_then(serde_json::Value::as_str),
        Some("schemas/**"),
        "workspace generated removal scope before"
    );
    assert!(
        change
            .pointer("/scope/after")
            .is_some_and(|value| value.is_null()),
        "workspace generated removal scope after should be null: {change:?}"
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
fn saved_diff_output_covers_owner_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-owner-added");
    fixture.write_panic_source();
    write_policy_with_optional_owner(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_owner(&fixture, Some("core/tests"));

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
        "diff owner addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff owner addition improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("owner_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-owner")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected owner addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "owner addition severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("owner"),
        "owner addition metadata field"
    );
    assert!(
        change
            .pointer("/metadata/before")
            .is_some_and(|value| value.is_null()),
        "owner addition metadata before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("core/tests"),
        "owner addition metadata after"
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
fn saved_diff_output_covers_reason_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-reason-added");
    fixture.write_panic_source();
    write_policy_with_optional_reason(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_reason(
        &fixture,
        Some("Fixture keeps saved diff reason-addition posture details covered."),
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
        "diff reason addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff reason addition improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("reason_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-reason")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected reason addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "reason addition severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("reason"),
        "reason addition metadata field"
    );
    assert!(
        change
            .pointer("/metadata/before")
            .is_some_and(|value| value.is_null()),
        "reason addition metadata before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("Fixture keeps saved diff reason-addition posture details covered."),
        "reason addition metadata after"
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
fn saved_diff_output_covers_classification_addition_details() {
    let fixture = SourceTreeFixture::new("saved-diff-classification-added");
    fixture.write_panic_source();
    write_policy_with_optional_classification(&fixture, None);
    commit_fixture_base(&fixture.root);
    write_policy_with_optional_classification(&fixture, Some("reviewed_fixture"));

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
        "diff classification addition net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "diff classification addition improvement count"
    );

    let changes = value
        .pointer("/diff/policy_changes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let change = changes
        .iter()
        .find(|change| {
            change.get("kind").and_then(serde_json::Value::as_str) == Some("classification_added")
                && change.get("allow_id").and_then(serde_json::Value::as_str)
                    == Some("allow-unwrap-classification")
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected classification addition policy change; got {changes:?}"
            ))
        });
    assert_eq!(
        change.get("severity").and_then(serde_json::Value::as_str),
        Some("improvement"),
        "classification addition severity"
    );
    assert_eq!(
        change
            .pointer("/metadata/field")
            .and_then(serde_json::Value::as_str),
        Some("classification"),
        "classification addition metadata field"
    );
    assert!(
        change
            .pointer("/metadata/before")
            .is_some_and(|value| value.is_null()),
        "classification addition metadata before should be null: {change:?}"
    );
    assert_eq!(
        change
            .pointer("/metadata/after")
            .and_then(serde_json::Value::as_str),
        Some("reviewed_fixture"),
        "classification addition metadata after"
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
review_after = "2027-08-29"

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

fn write_policy_with_optional_created(fixture: &SourceTreeFixture, created: Option<&str>) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let created = created
        .map(|created| format!("created = \"{created}\"\n"))
        .unwrap_or_default();
    // The entry must stay live for the expected-success diff, so the expiry
    // is computed relative to today instead of a hardcoded calendar date.
    let expires = allow_core::SimpleDate::today_utc_approx().add_days(60);
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-unwrap-lifecycle"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff created-date posture details covered."
evidence = ["test:saved_diff_output_covers_created_removal_details"]
{created}expires = "{expires}"
review_after = "2026-07-29"

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
    // The entry must stay live on both sides of the diff, and baseline_debt
    // expiry is validated against the 120-day window from creation, so the
    // dates are computed relative to today instead of hardcoded calendar
    // days the test would eventually sail past.
    let created = allow_core::SimpleDate::today_utc_approx().add_days(-30);
    let expires = allow_core::SimpleDate::today_utc_approx().add_days(30);
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
created = "{created}"
expires = "{expires}"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_reviewed_allow(fixture: &SourceTreeFixture) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(
        r#"

[[allow]]
id = "allow-added-review"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff added-allow posture details covered."
evidence = ["test:saved_diff_output_covers_added_allow_details"]
created = "2026-05-29"
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
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
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_policy_status(fixture: &SourceTreeFixture, status: &str) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy = policy.replace("status = \"active\"", &format!("status = \"{status}\""));
    policy.push_str(
        r#"

[[allow]]
id = "allow-policy-status-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff policy-status weakening posture details covered."
evidence = ["test:saved_diff_output_covers_policy_status_weakening_details"]
created = "2026-05-29"
review_after = "2027-08-29"

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
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_workspace_ignored(fixture: &SourceTreeFixture, ignored: &[&str]) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let ignored = ignored
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    policy = policy.replace(
        "ignored = [\"policy/**\", \"target/**\"]",
        &format!("ignored = [{ignored}]"),
    );
    policy.push_str(
        r#"

[[allow]]
id = "allow-workspace-ignored-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff workspace-ignored posture details covered."
evidence = ["test:saved_diff_output_covers_workspace_ignored_addition_details"]
created = "2026-05-29"
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_with_workspace_generated(fixture: &SourceTreeFixture, generated: &[&str]) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let generated = generated
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    policy = policy.replace(
        "generated = [\"target/**\", \"vendor/**\"]",
        &format!("generated = [{generated}]"),
    );
    policy.push_str(
        r#"

[[allow]]
id = "allow-workspace-generated-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff workspace-generated posture details covered."
evidence = ["test:saved_diff_output_covers_workspace_generated_addition_details"]
created = "2026-05-29"
review_after = "2027-08-29"

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
review_after = "2027-08-29"

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
review_after = "2027-08-29"

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
review_after = "2027-08-29"

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
    severity: &str,
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
        Some(severity),
        "lifecycle severity for {kind}"
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

fn write_diff_evidence_fixture_doc(fixture: &SourceTreeFixture, relative_path: &str) {
    let path = fixture.root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    }
    fs::write(path, "# Safety evidence\n\nFixture evidence artifact.\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence fixture: {err}")));
}

fn git_for_saved_diff(root: &std::path::Path, args: &[&str]) {
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

fn write_diff_traceability_fixture_doc(fixture: &SourceTreeFixture, relative_path: &str) {
    let path = fixture.root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|err| {
            std::panic::panic_any(format!("create traceability link dir: {err}"))
        });
    }
    fs::write(
        path,
        "# Traceability link\n\nFixture traceability artifact.\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write traceability link fixture: {err}")));
}

fn write_policy_with_optional_evidence(fixture: &SourceTreeFixture, evidence: Option<&str>) {
    write_policy_with_optional_evidence_inner(fixture, evidence, true);
}

fn write_policy_with_missing_optional_evidence(
    fixture: &SourceTreeFixture,
    evidence: Option<&str>,
) {
    write_policy_with_optional_evidence_inner(fixture, evidence, false);
}

fn write_policy_with_optional_evidence_inner(
    fixture: &SourceTreeFixture,
    evidence: Option<&str>,
    create_local_file: bool,
) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    if let Some(path) = evidence
        .filter(|_| create_local_file)
        .and_then(|reference| reference.strip_prefix("doc:"))
    {
        write_diff_evidence_fixture_doc(fixture, path);
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
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn root_policy_with_optional_evidence(evidence: Option<&str>) -> String {
    policy_with_optional_evidence_at_ignored_path(evidence, &["allow.toml", "policy/**"])
}

fn default_policy_with_optional_evidence(evidence: Option<&str>) -> String {
    policy_with_optional_evidence_at_ignored_path(evidence, &["policy/**"])
}

fn policy_with_optional_evidence_at_ignored_path(
    evidence: Option<&str>,
    ignored: &[&str],
) -> String {
    let evidence = evidence
        .map(|evidence| format!("evidence = [\"{evidence}\"]\n"))
        .unwrap_or_default();
    let ignored = ignored
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"policy = "cargo-allow"

[workspace]
ignored = [{ignored}]

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
{evidence}created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn append_evidence_doc_allow(fixture: &SourceTreeFixture, relative_path: &str) {
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-evidence-doc"
kind = "non_rust_file"
family = "documentation"
path = "{relative_path}"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps local evidence documents receipted while diffing evidence posture."
created = "2026-05-29"
review_after = "2027-08-29"

[allow.selector]
ast_kind = "tracked_file"
symbol = "{relative_path}"
target_fingerprint = "md"
glob = "{relative_path}"
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
        write_diff_evidence_fixture_doc(fixture, path);
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
review_after = "2027-08-29"

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
review_after = "2027-08-29"

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
review_after = "2027-08-29"

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
review_after = "2027-08-29"

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

fn write_policy_with_exception_identity(fixture: &SourceTreeFixture, kind: &str, family: &str) {
    fixture.write_minimal_policy();
    let mut policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    policy.push_str(&format!(
        r#"

[[allow]]
id = "allow-exception-identity"
kind = "{kind}"
family = "{family}"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps saved diff exception identity change details covered."
evidence = ["test:saved_diff_output_covers_exception_identity_change_details"]
created = "2026-05-29"
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
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
review_after = "2027-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    ));
    fs::write(fixture.root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}
