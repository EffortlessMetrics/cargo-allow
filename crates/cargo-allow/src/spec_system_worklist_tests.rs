use super::worklist_args::WorklistFormat;
use super::{WorklistArgs, cmd_worklist};
use crate::{ProfileArg, RootArgs};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn worklist_spec_system_profile_does_not_require_allow_policy() {
    let root = fixture_root("worklist-profile-ok");
    write_valid_spec_system_fixture(&root);
    let output = root.join("worklist.json");

    let result = cmd_worklist(&worklist_args(&root, &output));

    assert!(
        result.is_ok(),
        "spec-system profile worklist should pass: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json("worklist report", &report);

    assert_eq!(
        json.get("schema_id").and_then(Value::as_str),
        Some(allow_report::SPEC_SYSTEM_SCHEMA_ID)
    );
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("worklist")
    );
    assert_eq!(
        json.pointer("/summary/work_items").and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn worklist_spec_system_profile_reports_missing_node_for_missing_ledger() {
    let root = fixture_root("worklist-profile-missing-ledger");
    write_file(&root, "docs/status/SUPPORT_TIERS.md", support_tiers());
    let output = root.join("worklist.json");

    let result = cmd_worklist(&worklist_args(&root, &output));

    assert!(
        result.is_ok(),
        "missing ledger should be advisory worklist output: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json("worklist report", &report);

    assert!(
        has_work_item_kind(&json, "missing_node"),
        "missing ledger should route a missing_node work item: {report}"
    );
}

#[test]
fn worklist_spec_system_profile_reports_unknown_link_target() {
    let root = fixture_root("worklist-profile-unknown-link");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &valid_doc_artifact_ledger().replace(
            "linked_proposal = \"CARGO-ALLOW-PROP-0001\"",
            "linked_proposal = \"CARGO-ALLOW-PROP-MISSING\"",
        ),
    );
    let output = root.join("worklist.json");

    let result = cmd_worklist(&worklist_args(&root, &output));

    assert!(
        result.is_ok(),
        "unknown graph link should be advisory worklist output: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json("worklist report", &report);

    assert!(
        has_work_item_kind(&json, "unknown_link_target"),
        "unknown target should route an unknown_link_target work item: {report}"
    );
    assert!(
        work_items(&json).iter().any(|item| {
            item.get("artifact_id").and_then(Value::as_str) == Some("CARGO-ALLOW-SPEC-0001")
                && item
                    .get("proof_commands")
                    .and_then(Value::as_array)
                    .is_some_and(|commands| {
                        commands.iter().any(|command| {
                            command.as_str()
                                == Some("cargo-allow check --profile spec-system --mode audit")
                        })
                    })
        }),
        "unknown target should keep artifact context and proof commands: {report}"
    );
}

#[test]
fn worklist_spec_system_profile_reports_missing_closeout() {
    let root = fixture_root("worklist-profile-missing-closeout");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "policy/doc-artifacts.toml",
        &done_plan_ledger_without_closeout(),
    );
    let output = root.join("worklist.json");

    let result = cmd_worklist(&worklist_args(&root, &output));

    assert!(
        result.is_ok(),
        "missing closeout should be advisory worklist output: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json("worklist report", &report);

    assert!(
        has_work_item_kind(&json, "missing_closeout"),
        "done plan without closeout should route a missing_closeout work item: {report}"
    );
}

#[test]
fn worklist_spec_system_profile_reports_missing_proof_command() {
    let root = fixture_root("worklist-profile-missing-proof");
    write_valid_spec_system_fixture(&root);
    write_file(
        &root,
        "docs/status/SUPPORT_TIERS.md",
        &support_tiers_with_missing_stable_proof(),
    );
    let output = root.join("worklist.json");

    let result = cmd_worklist(&worklist_args(&root, &output));

    assert!(
        result.is_ok(),
        "missing proof command should be advisory worklist output: {:?}",
        result.err()
    );
    let report = read_to_string(&output);
    let _ = fs::remove_dir_all(&root);
    let json = parse_spec_system_json("worklist report", &report);

    assert!(
        has_work_item_kind(&json, "missing_proof_command"),
        "stable claim without proof should route a missing_proof_command work item: {report}"
    );
}

#[test]
fn worklist_spec_system_profile_rejects_source_exception_filters() {
    let root = fixture_root("worklist-profile-kind-filter");
    let output = root.join("worklist.json");
    let mut args = worklist_args(&root, &output);
    args.kind = Some("unsafe".to_string());

    let result = cmd_worklist(&args);
    let _ = fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("--kind is not supported"));
}

fn worklist_args(root: &Path, output: &Path) -> WorklistArgs {
    WorklistArgs {
        root: RootArgs {
            root: Some(root.to_path_buf()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        kind: None,
        family: None,
        item_kind: None,
        status: None,
        allow_id: None,
        path: None,
        source_package: None,
        owner: None,
        classification: None,
        baseline_debt: false,
        broad_scope: false,
        risk: None,
        difficulty: None,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
        include_untracked: false,
        format: WorklistFormat::Json,
        output: Some(output.to_path_buf()),
    }
}

fn has_work_item_kind(json: &Value, kind: &str) -> bool {
    work_items(json)
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some(kind))
}

fn work_items(json: &Value) -> Vec<Value> {
    json.get("work_items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn fixture_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-spec-system-worklist-{label}-{}-{}",
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

fn done_plan_ledger_without_closeout() -> String {
    valid_doc_artifact_ledger().replace(
        r#"status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_goal = "CARGO-ALLOW-GOAL-0001"
linked_closeout = "CARGO-ALLOW-CLOSEOUT-0001""#,
        r#"status = "done"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_goal = "CARGO-ALLOW-GOAL-0001""#,
    )
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

fn support_tiers_with_missing_stable_proof() -> String {
    support_tiers().replace(
        "| Source exception ledger | Stable | Source-tree findings are checked against policy. | cargo-allow check --mode no-new | Source-tree only. |",
        "| Source exception ledger | Stable | Source-tree findings are checked against policy. | | Source-tree only. |",
    )
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
