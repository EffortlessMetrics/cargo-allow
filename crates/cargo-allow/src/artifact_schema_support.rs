use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn governed_kind_enum() -> Vec<&'static str> {
    allow_core::FindingKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect()
}

pub(crate) fn match_status_enum() -> Vec<&'static str> {
    allow_core::MatchStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SchemaContract {
    pub(crate) name: &'static str,
    pub(crate) schema: &'static str,
    pub(crate) schema_id: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) fixed_command: Option<&'static str>,
}

pub(crate) fn schema_contracts() -> [SchemaContract; 10] {
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

pub(crate) fn parse_schema(name: &str, schema: &str) -> Value {
    serde_json::from_str(schema)
        .unwrap_or_else(|err| std::panic::panic_any(format!("{name} schema JSON: {err}")))
}

pub(crate) fn required_schema_pointer<'a>(
    name: &str,
    schema: &'a Value,
    pointer: &str,
) -> &'a Value {
    match schema.pointer(pointer) {
        Some(value) => value,
        None => std::panic::panic_any(format!("{name} schema should define {pointer}")),
    }
}

pub(crate) fn assert_required_fields(name: &str, schema: &Value, fields: &[&str]) {
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

pub(crate) fn assert_command_contract(contract: SchemaContract, schema: &Value) {
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

pub(crate) fn assert_inventory_schema(name: &str, schema: &Value) {
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

pub(crate) fn assert_enum_contains_all(
    name: &str,
    schema: &Value,
    pointer: &str,
    expected: &[&str],
) {
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

pub(crate) fn assert_enum_equals(name: &str, schema: &Value, pointer: &str, expected: &[&str]) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} {pointer} should be an enum array"));
    };
    let actual = items
        .iter()
        .map(|item| {
            item.as_str().unwrap_or_else(|| {
                std::panic::panic_any(format!("{name} {pointer} entries should be strings"))
            })
        })
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{name} enum values");
}

pub(crate) fn assert_schema_type_contains(
    name: &str,
    schema: &Value,
    pointer: &str,
    expected: &str,
) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} {pointer} should be a type array"));
    };
    assert!(
        items
            .iter()
            .any(|schema_item| schema_item.as_str() == Some(expected)),
        "{name} {pointer} should contain {expected}"
    );
}
