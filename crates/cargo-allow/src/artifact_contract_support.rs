use serde_json::Value;

pub(crate) fn parse_json_artifact(
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

pub(crate) fn assert_inventory_contract(
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

fn assert_json_array_contains(value: &Value, field: &str, expected: &str, artifact: &str) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{artifact} {field} should be an array"));
    };
    assert!(
        items.iter().any(|item| item.as_str() == Some(expected)),
        "{artifact} {field} should contain {expected}"
    );
}
