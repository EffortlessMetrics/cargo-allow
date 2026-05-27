use crate::{add, diff, doctor, explain, list, migrate, propose, prune, worklist};
use serde_json::Value;

#[test]
fn json_artifact_renderers_emit_parseable_v1_contracts() {
    let report_json = allow_report::render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        allow_report::ReportContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/source-snapshot"),
            inventory_files: Some(7),
            ..allow_report::ReportContext::default()
        },
    );
    let report = parse_json_artifact(
        "report",
        &report_json,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
    );
    assert_inventory_contract(
        "report",
        &report,
        "filesystem_fallback",
        Some("fixtures/source-snapshot"),
        Some(7),
    );

    let receipt_json = allow_report::render_receipt_with_context(
        "check",
        &[],
        false,
        allow_report::ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(42),
            ..allow_report::ReportContext::default()
        },
    );
    let receipt = parse_json_artifact(
        "receipt",
        &receipt_json,
        allow_report::RECEIPT_SCHEMA_ID,
        "check",
    );
    assert_inventory_contract(
        "receipt",
        &receipt,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(42),
    );

    let diff_base_json = allow_report::render_json_with_context(
        "diff",
        &[],
        &[],
        false,
        allow_report::ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(8),
            ..allow_report::ReportContext::default()
        },
    );
    let diff_json = diff::render_diff_json_with_posture(diff_base_json, &[], &[], &[]);
    let diff = parse_json_artifact("diff", &diff_json, allow_report::REPORT_SCHEMA_ID, "diff");
    assert_eq!(
        diff.pointer("/diff/net_posture").and_then(Value::as_str),
        Some("unchanged"),
        "diff net posture"
    );

    let list_json = list::sample_list_json_for_contract_test();
    let list = parse_json_artifact("list", &list_json, allow_report::LIST_SCHEMA_ID, "list");
    assert_eq!(
        list.pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(1),
        "list allow_entries"
    );

    let explain_json = explain::sample_explain_json_for_contract_test();
    let explain = parse_json_artifact(
        "explain",
        &explain_json,
        allow_report::EXPLAIN_SCHEMA_ID,
        "explain",
    );
    assert_eq!(
        explain.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-json"),
        "explain allow id"
    );

    let add_json = add::sample_add_json_for_contract_test();
    let add = parse_json_artifact("add", &add_json, allow_report::ADD_SCHEMA_ID, "add");
    assert_eq!(
        add.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-add-json"),
        "add allow id"
    );

    let worklist_json = worklist::sample_worklist_json_for_contract_test();
    let worklist = parse_json_artifact(
        "worklist",
        &worklist_json,
        allow_report::WORKLIST_SCHEMA_ID,
        "worklist",
    );
    assert_eq!(
        worklist
            .pointer("/summary/work_items")
            .and_then(Value::as_u64),
        Some(0),
        "worklist work_items"
    );

    let prune_json = prune::sample_prune_json_for_contract_test();
    let prune = parse_json_artifact("prune", &prune_json, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        prune
            .pointer("/summary/stale_entries")
            .and_then(Value::as_u64),
        Some(0),
        "prune stale_entries"
    );

    let propose_json = propose::sample_propose_json_for_contract_test();
    let propose = parse_json_artifact(
        "propose",
        &propose_json,
        allow_report::PROPOSE_SCHEMA_ID,
        "propose",
    );
    assert_eq!(
        propose
            .pointer("/summary/baseline_debt_entries_proposed")
            .and_then(Value::as_u64),
        Some(3),
        "propose baseline_debt_entries_proposed"
    );

    let migrate_json = migrate::sample_migrate_json_for_contract_test();
    let migrate = parse_json_artifact(
        "migrate",
        &migrate_json,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
    );
    assert_eq!(
        migrate
            .pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(1),
        "migrate allow_entries"
    );

    let doctor_json = doctor::sample_doctor_json_for_contract_test();
    let doctor = parse_json_artifact(
        "doctor",
        &doctor_json,
        allow_report::DOCTOR_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        doctor.pointer("/root/discovery").and_then(Value::as_str),
        Some("nearest_git_root"),
        "doctor root discovery"
    );
}

#[test]
fn report_schema_documents_diff_posture_contract() {
    let schema = include_str!("../../../docs/schemas/report.schema.json");

    assert!(schema.contains("\"diff\""));
    assert!(schema.contains("\"net_posture\""));
    assert!(schema.contains("\"finding_changes\""));
    assert!(schema.contains("\"policy_changes\""));
    assert!(schema.contains("\"scope_broadened\""));
    assert!(schema.contains("\"scope_narrowed\""));
    assert!(schema.contains("\"removed_allow\""));
    assert!(schema.contains("\"selector_precision_increased\""));
    assert!(schema.contains("\"evidence_added\""));
    assert!(schema.contains("\"expiry_shortened\""));
    assert!(schema.contains("\"review_after_shortened\""));
    assert!(schema.contains("\"owner_added\""));
    assert!(schema.contains("\"reason_added\""));
    assert!(schema.contains("\"classification_added\""));
    assert!(schema.contains("\"occurrence_limit_tightened\""));
    assert!(schema.contains("\"policy_improvements\""));
}

#[test]
fn prune_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/prune.schema.json");

    assert!(schema.contains(allow_report::PRUNE_SCHEMA_ID));
    assert!(schema.contains("\"mode\""));
    assert!(schema.contains("\"dry_run\""));
    assert!(schema.contains("\"written_path\""));
    assert!(schema.contains("\"stale_entries\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}

fn parse_json_artifact(
    name: &str,
    json: &str,
    expected_schema_id: &str,
    expected_command: &str,
) -> Value {
    let value: Value = serde_json::from_str(json).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "{name} artifact should parse as JSON: {err}\n{json}"
        ))
    });
    assert_eq!(
        value.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "{name} schema_version"
    );
    assert_eq!(
        value.get("schema_id").and_then(Value::as_str),
        Some(expected_schema_id),
        "{name} schema_id"
    );
    assert_eq!(
        value.get("command").and_then(Value::as_str),
        Some(expected_command),
        "{name} command"
    );
    assert_json_array_contains(&value, "claim_boundary", "source_tree_inventory", name);
    assert_json_array_contains(
        &value,
        "scanner_limitations",
        "cargo_metadata_not_invoked",
        name,
    );
    assert_json_array_contains(
        &value,
        "scanner_limitations",
        "repository_code_not_executed",
        name,
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree"),
        "{name} inventory scope"
    );
    assert_eq!(
        value
            .pointer("/inventory/scanner")
            .and_then(Value::as_str)
            .map(|scanner| scanner == "source_syntax" || scanner == "policy_migration"),
        Some(true),
        "{name} inventory scanner should be source_syntax or policy_migration"
    );
    value
}

fn assert_json_array_contains(value: &Value, field: &str, expected: &str, artifact: &str) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{artifact} {field} should be an array"));
    };
    assert!(
        items.iter().any(|item| item.as_str() == Some(expected)),
        "{artifact} {field} should contain {expected}"
    );
}

fn assert_inventory_contract(
    name: &str,
    value: &Value,
    expected_source: &str,
    expected_root: Option<&str>,
    expected_files: Option<u64>,
) {
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some(expected_source),
        "{name} inventory source"
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        expected_root,
        "{name} inventory root"
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        expected_files,
        "{name} inventory files_scanned"
    );
}
