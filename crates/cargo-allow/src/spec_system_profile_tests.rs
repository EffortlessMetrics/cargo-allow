use crate::{CargoAllowCli, CargoAllowCommand, OutputFormat, ProfileArg, RootArgs, audit, check};
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
    assert!(report.contains("No spec-system advisory findings."));
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
    assert!(!report.contains("No spec-system advisory findings."));
}

#[test]
fn check_spec_system_profile_rejects_source_exception_kind_filter() {
    let root = fixture_root("check-profile-kind-filter");
    write_valid_spec_system_fixture(&root);

    let result = check::cmd_check(&check::CheckArgs {
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
    write_file(root, ".codex/goals/active.toml", "CARGO-ALLOW-GOAL-0001\n");
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

fn doc_artifact_ledger_without_spec_proposal() -> String {
    valid_doc_artifact_ledger()
        .replace("linked_proposal = \"CARGO-ALLOW-PROP-0001\"\n\n[[artifact]]\nid = \"CARGO-ALLOW-SUPPORT-0001\"", "\n[[artifact]]\nid = \"CARGO-ALLOW-SUPPORT-0001\"")
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
    assert_eq!(
        value.get("failed").and_then(Value::as_bool),
        Some(false),
        "{name} failed"
    );
    value
}

fn unique_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
