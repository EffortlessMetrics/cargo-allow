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

pub(crate) fn inventory_source_enum() -> Vec<&'static str> {
    allow_inventory::InventorySource::ALL
        .iter()
        .map(|source| source.as_str())
        .chain(std::iter::once("unknown"))
        .collect()
}

pub(crate) fn enum_strings<T: Copy>(
    values: &[T],
    as_str: impl Fn(T) -> &'static str,
) -> Vec<&'static str> {
    values.iter().copied().map(as_str).collect()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SchemaContract {
    pub(crate) name: &'static str,
    pub(crate) schema: &'static str,
    pub(crate) schema_id: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) inventory_scanner: &'static str,
    pub(crate) fixed_command: Option<&'static str>,
}

pub(crate) fn schema_contracts() -> [SchemaContract; 16] {
    [
        schema_contract("add", include_str!("../../../docs/schemas/add.schema.json")),
        schema_contract(
            "add-finding-plan",
            include_str!("../../../docs/schemas/add-finding-plan.schema.json"),
        ),
        schema_contract(
            "add-plan-application",
            include_str!("../../../docs/schemas/add-plan-application.schema.json"),
        ),
        schema_contract(
            "core-adoption-plan",
            include_str!("../../../docs/schemas/core-adoption-plan.schema.json"),
        ),
        schema_contract(
            "doctor",
            include_str!("../../../docs/schemas/doctor.schema.json"),
        ),
        schema_contract(
            "explain",
            include_str!("../../../docs/schemas/explain.schema.json"),
        ),
        schema_contract(
            "list",
            include_str!("../../../docs/schemas/list.schema.json"),
        ),
        schema_contract(
            "migrate",
            include_str!("../../../docs/schemas/migrate.schema.json"),
        ),
        schema_contract(
            "propose",
            include_str!("../../../docs/schemas/propose.schema.json"),
        ),
        schema_contract(
            "prune",
            include_str!("../../../docs/schemas/prune.schema.json"),
        ),
        schema_contract(
            "refresh",
            include_str!("../../../docs/schemas/refresh.schema.json"),
        ),
        schema_contract(
            "receipt",
            include_str!("../../../docs/schemas/receipt.schema.json"),
        ),
        schema_contract(
            "report",
            include_str!("../../../docs/schemas/report.schema.json"),
        ),
        schema_contract(
            "spec-system",
            include_str!("../../../docs/schemas/spec-system.schema.json"),
        ),
        schema_contract("why", include_str!("../../../docs/schemas/why.schema.json")),
        schema_contract(
            "worklist",
            include_str!("../../../docs/schemas/worklist.schema.json"),
        ),
    ]
}

fn schema_contract(name: &'static str, schema: &'static str) -> SchemaContract {
    let artifact = allow_report::ARTIFACT_CONTRACTS
        .iter()
        .copied()
        .find(|contract| contract.name == name)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing artifact contract {name}")));
    SchemaContract {
        name: artifact.name,
        schema,
        schema_id: artifact.schema_id,
        schema_version: artifact.schema_version,
        inventory_scanner: artifact.inventory_scanner,
        fixed_command: artifact.fixed_command,
    }
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
    } else if schema
        .pointer("/properties/command/const")
        .and_then(Value::as_str)
        .is_some()
        || schema
            .pointer("/properties/command/enum")
            .and_then(Value::as_array)
            .is_some()
    {
        assert!(
            schema
                .pointer("/properties/command/const")
                .and_then(Value::as_str)
                .is_some_and(|command| !command.is_empty())
                || schema
                    .pointer("/properties/command/enum")
                    .and_then(Value::as_array)
                    .is_some_and(|commands| {
                        !commands.is_empty()
                            && commands
                                .iter()
                                .all(|command| command.as_str().is_some_and(|s| !s.is_empty()))
                    }),
            "{} command contract should name non-empty producer commands",
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

pub(crate) fn assert_inventory_schema(name: &str, schema: &Value, expected_scanner: &str) {
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
        Some(allow_report::INVENTORY_SCOPE_SOURCE_TREE),
        "{name} inventory scope"
    );
    let Some(scanner_schema) = inventory_schema.pointer("/properties/scanner") else {
        std::panic::panic_any(format!("{name} inventory scanner schema missing"));
    };
    assert_eq!(
        scanner_schema.get("const").and_then(Value::as_str),
        Some(expected_scanner),
        "{name} inventory scanner const"
    );
    assert_enum_equals(
        name,
        inventory_schema,
        "/properties/source/enum",
        &inventory_source_enum(),
    );
    assert_eq!(
        inventory_schema
            .pointer("/properties/root/type")
            .and_then(Value::as_str),
        Some("string"),
        "{name} inventory root type"
    );
    assert_eq!(
        inventory_schema
            .pointer("/properties/root/minLength")
            .and_then(Value::as_u64),
        Some(1),
        "{name} inventory root minLength"
    );
    assert_eq!(
        inventory_schema
            .pointer("/properties/empty_git_tracked/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "{name} inventory empty_git_tracked type"
    );
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

pub(crate) fn assert_schema_type_equals(
    name: &str,
    schema: &Value,
    pointer: &str,
    expected: &[&str],
) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} {pointer} should be a type array"));
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
    assert_eq!(actual, expected, "{name} {pointer} type values");
}
