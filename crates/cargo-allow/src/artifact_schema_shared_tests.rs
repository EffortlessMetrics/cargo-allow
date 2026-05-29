use crate::artifact_schema_support::{
    assert_command_contract, assert_enum_contains_all, assert_inventory_schema,
    assert_required_fields, parse_schema, schema_contracts,
};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

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

#[test]
fn schema_contract_registry_covers_every_documented_artifact_schema() {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
    let documented = fs::read_dir(&schema_dir)
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "read schema directory {}: {err}",
                schema_dir.display()
            ))
        })
        .map(|entry| {
            entry.unwrap_or_else(|err| {
                std::panic::panic_any(format!(
                    "read schema directory entry {}: {err}",
                    schema_dir.display()
                ))
            })
        })
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.strip_suffix(".schema.json")
                .map(std::string::ToString::to_string)
        })
        .collect::<BTreeSet<_>>();
    let registered = schema_contracts()
        .into_iter()
        .map(|contract| contract.name.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        registered, documented,
        "every docs/schemas/*.schema.json file should be registered for shared contract tests"
    );
}

#[test]
fn schema_files_reject_unknown_top_level_fields() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);

        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{} schema should reject unknown top-level fields",
            contract.name
        );
    }
}

#[test]
fn schema_files_keep_explicit_top_level_property_sets() {
    for (name, expected) in expected_top_level_schema_properties() {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == name)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")));
        let schema = parse_schema(contract.name, contract.schema);

        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            std::panic::panic_any(format!("{name} schema properties should be an object"));
        };
        let actual = properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{name} top-level schema properties");
    }
}

#[test]
fn schema_files_keep_explicit_top_level_required_sets() {
    for (name, expected) in expected_top_level_required_fields() {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == name)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")));
        let schema = parse_schema(contract.name, contract.schema);

        let Some(required) = schema.get("required").and_then(Value::as_array) else {
            std::panic::panic_any(format!("{name} schema required should be an array"));
        };
        let actual = required
            .iter()
            .map(|field| {
                field.as_str().unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{name} schema required entries should be strings"
                    ))
                })
            })
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{name} top-level required fields");
    }
}

#[test]
fn schema_object_nodes_reject_unknown_fields() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);
        let mut missing = Vec::new();

        collect_object_nodes_missing_additional_properties(&schema, "", &mut missing);

        assert!(
            missing.is_empty(),
            "{} object schemas should set additionalProperties=false at: {}",
            contract.name,
            missing.join(", ")
        );
    }
}

fn collect_object_nodes_missing_additional_properties(
    value: &Value,
    path: &str,
    missing: &mut Vec<String>,
) {
    match value {
        Value::Object(object) => {
            let has_properties = object.contains_key("properties");
            let is_object_type = object.get("type").and_then(Value::as_str) == Some("object");
            if (has_properties || is_object_type)
                && object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
            {
                missing.push(if path.is_empty() {
                    "/".to_string()
                } else {
                    path.to_string()
                });
            }
            for (key, child) in object {
                collect_object_nodes_missing_additional_properties(
                    child,
                    &format!("{path}/{}", json_pointer_escape(key)),
                    missing,
                );
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_object_nodes_missing_additional_properties(
                    child,
                    &format!("{path}/{index}"),
                    missing,
                );
            }
        }
        _ => {}
    }
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn expected_top_level_schema_properties() -> [(&'static str, &'static [&'static str]); 10] {
    [
        (
            "add",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "selected_finding",
                "summary",
                "tool",
            ],
        ),
        (
            "doctor",
            &[
                "claim_boundary",
                "command",
                "config",
                "inventory",
                "root",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "tool",
            ],
        ),
        (
            "explain",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "current_findings",
                "evidence_references",
                "inventory",
                "match_outcomes",
                "next",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "list",
            &[
                "allow_entries",
                "claim_boundary",
                "command",
                "filters",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "migrate",
            &[
                "claim_boundary",
                "command",
                "input",
                "inventory",
                "notes",
                "output",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "propose",
            &[
                "claim_boundary",
                "command",
                "generated_entry_defaults",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "prune",
            &[
                "claim_boundary",
                "command",
                "inventory",
                "mode",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "stale_entries",
                "summary",
                "tool",
            ],
        ),
        (
            "receipt",
            &[
                "claim_boundary",
                "command",
                "counts",
                "failed",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "tool",
            ],
        ),
        (
            "report",
            &[
                "claim_boundary",
                "command",
                "diff",
                "failed",
                "findings",
                "inventory",
                "outcomes",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "summary",
                "tool",
                "trend",
            ],
        ),
        (
            "worklist",
            &[
                "claim_boundary",
                "command",
                "filters",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
                "work_items",
            ],
        ),
    ]
}

fn expected_top_level_required_fields() -> [(&'static str, &'static [&'static str]); 10] {
    [
        (
            "add",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "selected_finding",
                "summary",
                "tool",
            ],
        ),
        (
            "doctor",
            &[
                "claim_boundary",
                "command",
                "config",
                "inventory",
                "root",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "tool",
            ],
        ),
        (
            "explain",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "current_findings",
                "evidence_references",
                "inventory",
                "match_outcomes",
                "next",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "list",
            &[
                "allow_entries",
                "claim_boundary",
                "command",
                "filters",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "migrate",
            &[
                "claim_boundary",
                "command",
                "input",
                "inventory",
                "notes",
                "output",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "propose",
            &[
                "claim_boundary",
                "command",
                "generated_entry_defaults",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "prune",
            &[
                "claim_boundary",
                "command",
                "inventory",
                "mode",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "stale_entries",
                "summary",
                "tool",
            ],
        ),
        (
            "receipt",
            &[
                "claim_boundary",
                "command",
                "counts",
                "failed",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "tool",
            ],
        ),
        (
            "report",
            &[
                "claim_boundary",
                "command",
                "failed",
                "findings",
                "inventory",
                "outcomes",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "summary",
                "tool",
            ],
        ),
        (
            "worklist",
            &[
                "claim_boundary",
                "command",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
                "work_items",
            ],
        ),
    ]
}
