use crate::{CargoAllowCli, CargoAllowCommand, OutputFormat, ProfileArg, RootArgs, audit, check};
use allow_core::CargoAllowResult;
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn clap_parses_spec_system_profile_for_check() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "check",
        "--profile",
        "spec-system",
    ]));
    assert!(
        parsed.is_ok(),
        "CLI should parse profile: {:?}",
        parsed.err()
    );
    let Ok(parsed) = parsed else {
        return;
    };

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Check(check::CheckArgs {
            artifact_dir: None,
            emit: None,
            profile: Some(ProfileArg::SpecSystem),
            ..
        }))
    ));
}

#[test]
fn clap_parses_spec_system_profile_for_audit() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "audit",
        "--profile",
        "spec-system",
    ]));
    assert!(
        parsed.is_ok(),
        "CLI should parse profile: {:?}",
        parsed.err()
    );
    let Ok(parsed) = parsed else {
        return;
    };

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Audit(audit::ReportArgs {
            artifact_dir: None,
            emit: None,
            profile: Some(ProfileArg::SpecSystem),
            ..
        }))
    ));
}

#[test]
fn clap_leaves_check_profile_unset_by_default() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check"]));
    assert!(parsed.is_ok(), "CLI should parse check: {:?}", parsed.err());
    let Ok(parsed) = parsed else {
        return;
    };

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Check(check::CheckArgs {
            artifact_dir: None,
            emit: None,
            profile: None,
            ..
        }))
    ));
}

#[test]
fn clap_rejects_unknown_profile() {
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check", "--profile", "docs"]));

    assert!(parsed.is_err());
    let Err(err) = parsed else {
        return;
    };
    assert!(err.to_string().contains("spec-system"));
}

#[test]
fn check_spec_system_profile_does_not_require_allow_policy() {
    let root = fixture_root("check-profile-ok");
    write_valid_spec_system_fixture(&root);
    let output = root.join("check.md");
    let receipt = root.join("receipt.json");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Markdown,
        output: Some(output.clone()),
        receipt: Some(receipt.clone()),
        mode: Some("audit".to_string()),
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });

    assert!(
        result.is_ok(),
        "spec-system profile check should pass: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let receipt_text = read_to_string(&receipt);
    let _ = fs::remove_dir_all(&root);

    assert!(report.contains("cargo-allow check --profile spec-system"));
    assert!(report.contains("No spec-system findings in `advisory` mode."));
    let receipt_json = parse_spec_system_json("receipt", &receipt_text);

    assert_eq!(
        receipt_json.get("schema_id").and_then(Value::as_str),
        Some(allow_report::SPEC_SYSTEM_SCHEMA_ID)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/artifacts")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/links")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/work_items")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/blocking_eligible_findings")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/advisory_findings")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/blocking_eligible_work_items")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        receipt_json
            .pointer("/summary/advisory_work_items")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert!(
        receipt_json
            .get("work_items")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    );
    assert_eq!(
        receipt_json
            .pointer("/links/0/field")
            .and_then(Value::as_str),
        Some("linked_proposal")
    );
}

#[test]
fn check_spec_system_profile_reports_explicit_missing_config() {
    let root = fixture_root("check-profile-missing-config");
    write_valid_spec_system_fixture(&root);
    let output = root.join("check.md");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(PathBuf::from("policy/missing-spec-system.toml")),
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Markdown,
        output: Some(output.clone()),
        receipt: None,
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });

    assert!(
        result.is_ok(),
        "missing explicit profile config should be advisory: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);

    assert!(report.contains("profile_config"));
    assert!(report.contains("does not exist"));
    assert!(!report.contains("No spec-system findings in `advisory` mode."));
}

#[test]
fn check_spec_system_profile_rejects_source_exception_kind_filter() {
    let root = fixture_root("check-profile-kind-filter");
    write_valid_spec_system_fixture(&root);

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: Some("unsafe".to_string()),
        include_untracked: false,
        format: OutputFormat::Markdown,
        output: None,
        receipt: None,
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });
    let _ = fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("--kind is not supported"));
}

#[test]
fn audit_spec_system_profile_does_not_require_allow_policy() {
    let root = fixture_root("audit-profile-ok");
    write_valid_spec_system_fixture(&root);
    let output = root.join("audit.json");

    let result = audit::cmd_audit(&audit::ReportArgs {
        artifact_dir: None,
        emit: None,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Json,
        output: Some(output.clone()),
    });

    assert!(
        result.is_ok(),
        "spec-system profile audit should pass: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);

    assert!(report.contains("\"command\": \"audit\""));
    assert!(report.contains("\"profile\": \"spec-system\""));
    assert!(report.contains(allow_report::SPEC_SYSTEM_SCHEMA_ID));
    assert!(report.contains("\"findings\": 0"));
}

#[test]
fn check_spec_system_profile_json_report_uses_v1_graph_artifact() {
    let root = fixture_root("check-profile-json-v1");
    write_valid_spec_system_fixture(&root);
    let output = root.join("check.json");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Json,
        output: Some(output.clone()),
        receipt: None,
        mode: Some("audit".to_string()),
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });

    assert!(
        result.is_ok(),
        "spec-system profile JSON check should pass: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json("check report", &report);

    assert_eq!(
        json.get("schema_id").and_then(Value::as_str),
        Some(allow_report::SPEC_SYSTEM_SCHEMA_ID)
    );
    assert_eq!(
        json.pointer("/inventory/scanner").and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCANNER_SOURCE_TREE_GRAPH)
    );
    assert!(
        json.get("claim_boundary")
            .and_then(Value::as_array)
            .is_some_and(|items| items
                .iter()
                .any(|item| { item.as_str() == Some("source_tree_graph_validation") }))
    );
    assert!(
        json.get("scanner_limitations")
            .and_then(Value::as_array)
            .is_some_and(|items| items
                .iter()
                .any(|item| { item.as_str() == Some("proof_commands_not_executed") }))
    );
    assert_eq!(
        json.pointer("/artifacts/1/id").and_then(Value::as_str),
        Some("CARGO-ALLOW-SPEC-0001")
    );
    assert_eq!(
        json.pointer("/artifacts/1/kind").and_then(Value::as_str),
        Some("spec")
    );
    assert_eq!(
        json.pointer("/links/0/target_kind").and_then(Value::as_str),
        Some("proposal")
    );
}

#[test]
fn spec_system_profile_reports_advisory_findings_without_failing() {
    let root = fixture_root("profile-advisory-finding");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &doc_artifact_ledger_without_spec_proposal(),
    );
    let output = root.join("check.md");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Markdown,
        output: Some(output.clone()),
        receipt: None,
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });

    assert!(
        result.is_ok(),
        "advisory spec-system findings should not fail check: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);

    assert!(report.contains("| Advisory findings | 1 |"));
    assert!(report.contains("requires linked_proposal"));
}

#[test]
fn spec_system_profile_reports_shadow_mode_without_failing_command() {
    let root = fixture_root("profile-shadow-finding");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("shadow"),
    );
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &doc_artifact_ledger_without_spec_proposal(),
    );
    let output = root.join("check.json");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Json,
        output: Some(output.clone()),
        receipt: None,
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });

    assert!(
        result.is_ok(),
        "shadow spec-system findings should report posture without failing command: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("shadow report", &report);

    assert_eq!(json.get("mode").and_then(Value::as_str), Some("shadow"));
    assert_eq!(json.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(json.get("failed").and_then(Value::as_bool), Some(true));
    assert_eq!(
        json.pointer("/summary/findings").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/summary/blocking_eligible_findings")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/summary/advisory_findings")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn spec_system_blocking_mode_fails_missing_artifact_file_after_writing_report() {
    let root = fixture_root("profile-blocking-missing-file");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("blocking"),
    );
    let missing = root.join("docs/specs/CARGO-ALLOW-SPEC-0001-example.md");
    let removed = fs::remove_file(&missing);
    assert!(
        removed.is_ok(),
        "fixture artifact should be removed: {:?}",
        removed.err()
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(
        result.is_err(),
        "blocking missing artifact file should fail command"
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("blocking missing file", &report);

    assert_blocking_report(&json, "artifact_file", "artifact_file_missing");
}

#[test]
fn spec_system_blocking_mode_fails_unknown_link_after_writing_report() {
    let root = fixture_root("profile-blocking-unknown-link");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("blocking"),
    );
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &doc_artifact_ledger_with_unknown_spec_proposal(),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(result.is_err(), "blocking unknown link should fail command");
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("blocking unknown link", &report);

    assert_blocking_report(&json, "artifact_link", "unknown_link_target");
}

#[test]
fn spec_system_blocking_mode_fails_duplicate_artifact_id_after_writing_report() {
    let root = fixture_root("profile-blocking-duplicate-id");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("blocking"),
    );
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &doc_artifact_ledger_with_duplicate_id(),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(result.is_err(), "blocking duplicate id should fail command");
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("blocking duplicate id", &report);

    assert_blocking_report(&json, "doc_artifact_ledger", "duplicate_id");
}

#[test]
fn spec_system_blocking_mode_fails_invalid_artifact_status_after_writing_report() {
    let root = fixture_root("profile-blocking-invalid-status");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("blocking"),
    );
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &doc_artifact_ledger_with_invalid_status(),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(
        result.is_err(),
        "blocking invalid artifact status should fail command"
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("blocking invalid status", &report);

    assert_blocking_report(
        &json,
        "doc_artifact_ledger",
        "invalid_artifact_kind_or_status",
    );
}

#[test]
fn spec_system_profile_malformed_config_fails_after_writing_report() {
    let root = fixture_root("profile-malformed-config");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        "schema_version = \"1.0\"\nprofile = \"spec-system\"\nmode = \"blocking\"\n[roots\n",
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(
        result.is_err(),
        "malformed profile config should fail as a setup error"
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("malformed config", &report);

    assert_eq!(json.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(json.get("failed").and_then(Value::as_bool), Some(true));
    assert_blocking_finding(&json, "profile_config", "profile_config_parse_failure");
}

#[test]
fn spec_system_blocking_mode_keeps_missing_required_edge_advisory() {
    let root = fixture_root("profile-blocking-missing-edge");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("blocking"),
    );
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &doc_artifact_ledger_without_spec_proposal(),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(
        result.is_ok(),
        "blocking mode should not command-block nuanced required-edge findings: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("blocking missing edge", &report);

    assert_eq!(json.get("mode").and_then(Value::as_str), Some("blocking"));
    assert_eq!(json.get("status").and_then(Value::as_str), Some("passed"));
    assert_eq!(json.get("failed").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.pointer("/summary/findings").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/summary/blocking_eligible_findings")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/summary/advisory_findings")
            .and_then(Value::as_u64),
        Some(1)
    );
    let finding = first_finding(&json);
    assert_eq!(
        finding.get("kind").and_then(Value::as_str),
        Some("artifact_link")
    );
    assert_eq!(
        finding.get("blocking_eligible").and_then(Value::as_bool),
        Some(false)
    );
    assert!(finding.get("blocking_reason").is_none());
}

#[test]
fn spec_system_blocking_mode_keeps_active_goal_manifest_findings_advisory() {
    let root = fixture_root("profile-blocking-active-goal-advisory");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("blocking"),
    );
    write_file(
        &root,
        ".codex/goals/active.toml",
        &valid_active_goal_manifest().replace(
            r#"proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
  "cargo-allow worklist --profile spec-system --format json",
]"#,
            "proof_commands = []",
        ),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);

    assert!(
        result.is_ok(),
        "blocking mode should not command-block active-goal lifecycle findings: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json_without_failed_assert("blocking active goal", &report);

    assert_eq!(json.get("mode").and_then(Value::as_str), Some("blocking"));
    assert_eq!(json.get("status").and_then(Value::as_str), Some("passed"));
    assert_eq!(json.get("failed").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.pointer("/summary/findings").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/summary/blocking_eligible_findings")
            .and_then(Value::as_u64),
        Some(0)
    );
    let finding = first_finding(&json);
    assert_eq!(
        finding.get("kind").and_then(Value::as_str),
        Some("active_goal")
    );
    assert_eq!(
        finding.get("blocking_eligible").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn spec_system_profile_renders_configured_shadow_mode_in_markdown() {
    let root = fixture_root("profile-shadow-markdown");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("shadow"),
    );
    let output = root.join("check.md");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Markdown,
        output: Some(output.clone()),
        receipt: None,
        // No `--mode` override: this asserts the shadow *config* renders as
        // shadow. (An explicit `--mode` now overrides the config mode, #1941.)
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });

    assert!(
        result.is_ok(),
        "shadow spec-system clean report should pass: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);

    assert!(report.contains("**Result:** shadow"));
    assert!(report.contains("Mode: `shadow`"));
    assert!(report.contains("Status: `passed`"));
    assert!(report.contains("| Findings | 0 |"));
    assert!(report.contains("No spec-system findings in `shadow` mode."));
}

#[test]
fn profile_resolution_uses_allow_profiles_config() {
    let root = fixture_root("allow-profiles-only");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        ".allow/profiles/spec-system.toml",
        &spec_system_config("advisory"),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);
    assert!(
        result.is_ok(),
        "allow profile config should pass: {:?}",
        result.err()
    );
    let json = parse_spec_system_json("allow profiles", &read_to_string(&output));
    let _ = fs::remove_dir_all(&root);

    assert_eq!(
        json.get("config_source").and_then(Value::as_str),
        Some(".allow/profiles/spec-system.toml")
    );
    assert_eq!(
        json.get("config_provenance").and_then(Value::as_str),
        Some("allow_profiles")
    );
}

#[test]
fn profile_resolution_falls_back_to_legacy_policy_config() {
    let root = fixture_root("legacy-policy-only");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("advisory"),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);
    assert!(
        result.is_ok(),
        "legacy profile config should pass: {:?}",
        result.err()
    );
    let json = parse_spec_system_json("legacy policy", &read_to_string(&output));
    let _ = fs::remove_dir_all(&root);

    assert_eq!(
        json.get("config_source").and_then(Value::as_str),
        Some("policy/spec-system.toml")
    );
    assert_eq!(
        json.get("config_provenance").and_then(Value::as_str),
        Some("legacy_policy")
    );
}

#[test]
fn profile_resolution_reports_owned_and_legacy_conflict() {
    let root = fixture_root("allow-legacy-conflict");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        ".allow/profiles/spec-system.toml",
        &spec_system_config("advisory"),
    );
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("shadow"),
    );
    let output = root.join("check.json");

    let result = spec_system_check_json(&root, &output);
    assert!(
        result.is_ok(),
        "owned/legacy conflict should remain advisory: {:?}",
        result.err()
    );
    let json = parse_spec_system_json("allow legacy conflict", &read_to_string(&output));
    let _ = fs::remove_dir_all(&root);

    assert_eq!(
        json.get("config_source").and_then(Value::as_str),
        Some(".allow/profiles/spec-system.toml")
    );
    assert_eq!(
        json.get("config_provenance").and_then(Value::as_str),
        Some("allow_profiles")
    );
    assert!(
        json.get("findings")
            .and_then(Value::as_array)
            .is_some_and(|findings| findings.iter().any(|finding| {
                finding.get("kind").and_then(Value::as_str) == Some("profile_config")
                    && finding
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| {
                            message.contains(".allow/profiles/spec-system.toml")
                                && message.contains("policy/spec-system.toml")
                        })
            })),
        "conflict diagnostic should name both configs: {json}"
    );
}

#[test]
fn profile_resolution_honors_explicit_config_override() {
    let root = fixture_root("explicit-config-override");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        ".allow/profiles/spec-system.toml",
        &spec_system_config("shadow"),
    );
    write_file(
        &root,
        "policy/spec-system.toml",
        &spec_system_config("shadow"),
    );
    write_file(
        &root,
        "custom/spec-system.toml",
        &spec_system_config("advisory"),
    );
    let output = root.join("check.json");

    let result = check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(PathBuf::from("custom/spec-system.toml")),
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Json,
        output: Some(output.clone()),
        receipt: None,
        mode: Some("audit".to_string()),
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });
    assert!(
        result.is_ok(),
        "explicit config override should pass: {:?}",
        result.err()
    );
    let json = parse_spec_system_json("explicit override", &read_to_string(&output));
    let _ = fs::remove_dir_all(&root);

    assert_eq!(
        json.get("config_source").and_then(Value::as_str),
        Some("custom/spec-system.toml")
    );
    assert_eq!(
        json.get("config_provenance").and_then(Value::as_str),
        Some("explicit_config")
    );
    assert!(
        json.get("findings")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty),
        "explicit override should not emit owned/legacy conflict: {json}"
    );
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn fixture_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-spec-system-profile-{label}-{}-{}",
        std::process::id(),
        unique_stamp()
    ));
    let result = fs::create_dir_all(&root);
    assert!(
        result.is_ok(),
        "fixture root should be created: {:?}",
        result.err()
    );
    root
}

fn write_valid_spec_system_fixture(root: &Path) {
    write_file(
        root,
        "policy/spec-system.toml",
        &spec_system_config("advisory"),
    );
    write_file(
        root,
        "policy/doc-artifacts.toml",
        &valid_doc_artifact_ledger(),
    );
    write_file(
        root,
        "docs/proposals/CARGO-ALLOW-PROP-0001-example.md",
        "CARGO-ALLOW-PROP-0001\n",
    );
    write_file(
        root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-example.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    write_file(root, "docs/status/SUPPORT_TIERS.md", support_tiers());
    write_file(
        root,
        ".codex/goals/active.toml",
        valid_active_goal_manifest(),
    );
    write_file(
        root,
        "plans/spec-system/implementation-plan.md",
        "CARGO-ALLOW-PLAN-0001\n",
    );
    write_file(
        root,
        "plans/spec-system/closeout.md",
        "CARGO-ALLOW-CLOSEOUT-0001\n",
    );
}

fn spec_system_check_json(root: &Path, output: &Path) -> CargoAllowResult<()> {
    check::cmd_check(&check::CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: check::PersistentCacheMode::On,
        root: RootArgs {
            root: Some(root.to_path_buf()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Json,
        output: Some(output.to_path_buf()),
        receipt: None,
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    })
}

fn spec_system_config(mode: &str) -> String {
    format!(
        r#"
schema_version = "1.0"
profile = "spec-system"
mode = "{mode}"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".codex/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = "policy/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
active_goal_required = true
closeout_required_for_done_items = true
"#
    )
}

fn valid_doc_artifact_ledger() -> String {
    r#"
schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"

[[artifact]]
id = "CARGO-ALLOW-PROP-0001"
kind = "proposal"
path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"

[[artifact]]
id = "CARGO-ALLOW-SPEC-0001"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"

[[artifact]]
id = "CARGO-ALLOW-SUPPORT-0001"
kind = "support_tier"
path = "docs/status/SUPPORT_TIERS.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"

[[artifact]]
id = "CARGO-ALLOW-GOAL-0001"
kind = "active_goal"
path = ".codex/goals/active.toml"
status = "active"
owner = "codex"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_plan = "plans/spec-system/implementation-plan.md"

[[artifact]]
id = "CARGO-ALLOW-PLAN-0001"
kind = "implementation_plan"
path = "plans/spec-system/implementation-plan.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_goal = "CARGO-ALLOW-GOAL-0001"
linked_closeout = "CARGO-ALLOW-CLOSEOUT-0001"

[[artifact]]
id = "CARGO-ALLOW-CLOSEOUT-0001"
kind = "closeout"
path = "plans/spec-system/closeout.md"
status = "draft"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_goal = "CARGO-ALLOW-GOAL-0001"
linked_plan = "CARGO-ALLOW-PLAN-0001"
"#
    .to_string()
}

fn valid_active_goal_manifest() -> &'static str {
    r#"
schema_version = "1.0"
id = "CARGO-ALLOW-GOAL-0001"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
linked_plan_status = "active"

[[work_item]]
id = "spec-system-pr-001"
status = "ready"
title = "Keep spec-system graph valid"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
  "cargo-allow worklist --profile spec-system --format json",
]
"#
}

fn doc_artifact_ledger_without_spec_proposal() -> String {
    valid_doc_artifact_ledger()
        .replace("linked_proposal = \"CARGO-ALLOW-PROP-0001\"\n\n[[artifact]]\nid = \"CARGO-ALLOW-SUPPORT-0001\"", "\n[[artifact]]\nid = \"CARGO-ALLOW-SUPPORT-0001\"")
}

fn doc_artifact_ledger_with_unknown_spec_proposal() -> String {
    valid_doc_artifact_ledger().replace(
        "linked_proposal = \"CARGO-ALLOW-PROP-0001\"\n\n[[artifact]]\nid = \"CARGO-ALLOW-SUPPORT-0001\"",
        "linked_proposal = \"CARGO-ALLOW-PROP-9999\"\n\n[[artifact]]\nid = \"CARGO-ALLOW-SUPPORT-0001\"",
    )
}

fn doc_artifact_ledger_with_duplicate_id() -> String {
    valid_doc_artifact_ledger().replacen(
        "id = \"CARGO-ALLOW-SPEC-0001\"",
        "id = \"CARGO-ALLOW-PROP-0001\"",
        1,
    )
}

fn doc_artifact_ledger_with_invalid_status() -> String {
    valid_doc_artifact_ledger().replacen("status = \"accepted\"", "status = \"unknown\"", 1)
}

fn support_tiers() -> &'static str {
    r#"
# Support Tiers

CARGO-ALLOW-SUPPORT-0001

| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| Source exception ledger | Stable | Source-tree findings are checked against policy. | cargo-allow check --mode no-new | Source-tree only. |
| Spec-system profile | Advisory | The repo carries planned graph artifacts. | | Advisory row. |
"#
}

fn write_file(root: &Path, relative_path: &str, contents: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        let result = fs::create_dir_all(parent);
        assert!(
            result.is_ok(),
            "fixture parent should be created: {:?}",
            result.err()
        );
    }
    let result = fs::write(&path, contents);
    assert!(
        result.is_ok(),
        "fixture file should be written: {:?}",
        result.err()
    );
}

fn read_to_string(path: &Path) -> String {
    let result = fs::read_to_string(path);
    assert!(
        result.is_ok(),
        "fixture file should be read: {:?}",
        result.err()
    );
    let Ok(text) = result else {
        return String::new();
    };
    text
}

fn parse_spec_system_json(name: &str, text: &str) -> Value {
    let value = parse_spec_system_json_without_failed_assert(name, text);
    assert_eq!(
        value.get("failed").and_then(Value::as_bool),
        Some(false),
        "{name} failed"
    );
    value
}

fn parse_spec_system_json_without_failed_assert(name: &str, text: &str) -> Value {
    let result = serde_json::from_str::<Value>(text);
    assert!(
        result.is_ok(),
        "{name} should parse as JSON: {:?}\n{text}",
        result.err()
    );
    let Ok(value) = result else {
        return Value::Null;
    };
    assert_eq!(
        value.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "{name} schema_version"
    );
    assert_eq!(
        value.get("tool").and_then(Value::as_str),
        Some("cargo-allow"),
        "{name} tool"
    );
    assert_eq!(
        value.get("profile").and_then(Value::as_str),
        Some("spec-system"),
        "{name} profile"
    );
    value
}

fn assert_blocking_report(json: &Value, finding_kind: &str, blocking_reason: &str) {
    assert_eq!(json.get("mode").and_then(Value::as_str), Some("blocking"));
    assert_eq!(json.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(json.get("failed").and_then(Value::as_bool), Some(true));
    assert_eq!(
        json.pointer("/summary/findings").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/summary/blocking_eligible_findings")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/summary/advisory_findings")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_blocking_finding(json, finding_kind, blocking_reason);
}

fn assert_blocking_finding(json: &Value, finding_kind: &str, blocking_reason: &str) {
    let finding = first_finding(json);
    assert_eq!(
        finding.get("kind").and_then(Value::as_str),
        Some(finding_kind)
    );
    assert_eq!(
        finding.get("blocking_eligible").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        finding.get("blocking_reason").and_then(Value::as_str),
        Some(blocking_reason)
    );
}

fn first_finding(json: &Value) -> &Value {
    let finding = json
        .get("findings")
        .and_then(Value::as_array)
        .and_then(|findings| findings.first());
    assert!(
        finding.is_some(),
        "spec-system report should include at least one finding"
    );
    let Some(finding) = finding else {
        return json;
    };
    finding
}

fn unique_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
