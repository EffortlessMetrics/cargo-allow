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

#[test]
fn schema_files_require_common_v1_source_tree_contract() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);

        assert_eq!(
            schema
                .pointer("/properties/schema_version/const")
                .and_then(Value::as_u64),
            Some(u64::from(contract.schema_version)),
            "{} schema_version const",
            contract.name
        );
        assert_eq!(
            schema
                .pointer("/properties/schema_id/const")
                .and_then(Value::as_str),
            Some(contract.schema_id),
            "{} schema_id const",
            contract.name
        );
        assert_required_fields(
            contract.name,
            &schema,
            &[
                "schema_version",
                "schema_id",
                "tool",
                "command",
                "claim_boundary",
                "scanner_limitations",
                "inventory",
            ],
        );
        assert_eq!(
            schema
                .pointer("/properties/tool/const")
                .and_then(Value::as_str),
            Some("cargo-allow"),
            "{} tool const",
            contract.name
        );
        assert_command_contract(contract, &schema);
        assert_inventory_schema(contract.name, &schema);
        assert_enum_contains_all(
            contract.name,
            &schema,
            "/$defs/claim_boundary_flag/enum",
            allow_report::CLAIM_BOUNDARY,
        );
        assert_enum_contains_all(
            contract.name,
            &schema,
            "/$defs/scanner_limitation/enum",
            allow_report::SCANNER_LIMITATIONS,
        );
    }
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

#[derive(Debug, Clone, Copy)]
struct SchemaContract {
    name: &'static str,
    schema: &'static str,
    schema_id: &'static str,
    schema_version: u32,
    fixed_command: Option<&'static str>,
}

fn schema_contracts() -> [SchemaContract; 10] {
    [
        SchemaContract {
            name: "add",
            schema: include_str!("../../../docs/schemas/add.schema.json"),
            schema_id: allow_report::ADD_SCHEMA_ID,
            schema_version: allow_report::ADD_SCHEMA_VERSION,
            fixed_command: Some("add"),
        },
        SchemaContract {
            name: "doctor",
            schema: include_str!("../../../docs/schemas/doctor.schema.json"),
            schema_id: allow_report::DOCTOR_SCHEMA_ID,
            schema_version: allow_report::DOCTOR_SCHEMA_VERSION,
            fixed_command: Some("doctor"),
        },
        SchemaContract {
            name: "explain",
            schema: include_str!("../../../docs/schemas/explain.schema.json"),
            schema_id: allow_report::EXPLAIN_SCHEMA_ID,
            schema_version: allow_report::EXPLAIN_SCHEMA_VERSION,
            fixed_command: Some("explain"),
        },
        SchemaContract {
            name: "list",
            schema: include_str!("../../../docs/schemas/list.schema.json"),
            schema_id: allow_report::LIST_SCHEMA_ID,
            schema_version: allow_report::LIST_SCHEMA_VERSION,
            fixed_command: Some("list"),
        },
        SchemaContract {
            name: "migrate",
            schema: include_str!("../../../docs/schemas/migrate.schema.json"),
            schema_id: allow_report::MIGRATE_SCHEMA_ID,
            schema_version: allow_report::MIGRATE_SCHEMA_VERSION,
            fixed_command: Some("migrate"),
        },
        SchemaContract {
            name: "propose",
            schema: include_str!("../../../docs/schemas/propose.schema.json"),
            schema_id: allow_report::PROPOSE_SCHEMA_ID,
            schema_version: allow_report::PROPOSE_SCHEMA_VERSION,
            fixed_command: Some("propose"),
        },
        SchemaContract {
            name: "prune",
            schema: include_str!("../../../docs/schemas/prune.schema.json"),
            schema_id: allow_report::PRUNE_SCHEMA_ID,
            schema_version: allow_report::PRUNE_SCHEMA_VERSION,
            fixed_command: Some("prune"),
        },
        SchemaContract {
            name: "receipt",
            schema: include_str!("../../../docs/schemas/receipt.schema.json"),
            schema_id: allow_report::RECEIPT_SCHEMA_ID,
            schema_version: allow_report::RECEIPT_SCHEMA_VERSION,
            fixed_command: None,
        },
        SchemaContract {
            name: "report",
            schema: include_str!("../../../docs/schemas/report.schema.json"),
            schema_id: allow_report::REPORT_SCHEMA_ID,
            schema_version: allow_report::REPORT_SCHEMA_VERSION,
            fixed_command: None,
        },
        SchemaContract {
            name: "worklist",
            schema: include_str!("../../../docs/schemas/worklist.schema.json"),
            schema_id: allow_report::WORKLIST_SCHEMA_ID,
            schema_version: allow_report::WORKLIST_SCHEMA_VERSION,
            fixed_command: Some("worklist"),
        },
    ]
}

fn parse_schema(name: &str, schema: &str) -> Value {
    serde_json::from_str(schema)
        .unwrap_or_else(|err| std::panic::panic_any(format!("{name} schema JSON: {err}")))
}

fn assert_required_fields(name: &str, schema: &Value, fields: &[&str]) {
    let Some(required) = schema.get("required").and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} schema required should be an array"));
    };
    for field in fields {
        assert!(
            required.iter().any(|item| item.as_str() == Some(*field)),
            "{name} schema should require {field}"
        );
    }
}

fn assert_command_contract(contract: SchemaContract, schema: &Value) {
    if let Some(command) = contract.fixed_command {
        assert_eq!(
            schema
                .pointer("/properties/command/const")
                .and_then(Value::as_str),
            Some(command),
            "{} command const",
            contract.name
        );
    } else {
        assert_eq!(
            schema
                .pointer("/properties/command/type")
                .and_then(Value::as_str),
            Some("string"),
            "{} command type",
            contract.name
        );
        assert_eq!(
            schema
                .pointer("/properties/command/minLength")
                .and_then(Value::as_u64),
            Some(1),
            "{} command minLength",
            contract.name
        );
    }
}

fn assert_inventory_schema(name: &str, schema: &Value) {
    let inventory_schema = schema
        .pointer("/$defs/inventory")
        .or_else(|| schema.pointer("/properties/inventory"))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{name} inventory schema should be defined"))
        });
    assert_eq!(
        inventory_schema
            .pointer("/properties/scope/const")
            .and_then(Value::as_str),
        Some("source_tree"),
        "{name} inventory scope"
    );
    let Some(scanner_schema) = inventory_schema.pointer("/properties/scanner") else {
        std::panic::panic_any(format!("{name} inventory scanner schema missing"));
    };
    let scanner_const = scanner_schema.get("const").and_then(Value::as_str);
    let scanner_enum_contains = |expected| {
        scanner_schema
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(expected)))
    };
    let scanner_matches_contract =
        matches!(scanner_const, Some("source_syntax" | "policy_migration"))
            || scanner_enum_contains("source_syntax")
            || scanner_enum_contains("policy_migration");
    assert!(
        scanner_matches_contract,
        "{name} inventory scanner should identify source_syntax or policy_migration"
    );
}

fn assert_enum_contains_all(name: &str, schema: &Value, pointer: &str, expected: &[&str]) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} schema {pointer} should be an array"));
    };
    for expected_item in expected {
        assert!(
            items
                .iter()
                .any(|schema_item| schema_item.as_str() == Some(*expected_item)),
            "{name} schema {pointer} should contain {expected_item}"
        );
    }
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
