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
    assert_json_array_eq(&value, "claim_boundary", allow_report::CLAIM_BOUNDARY, name);
    assert_json_array_eq(
        &value,
        "scanner_limitations",
        allow_report::SCANNER_LIMITATIONS,
        name,
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCOPE_SOURCE_TREE),
        "{name} inventory scope"
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some(expected_inventory_scanner(name, expected_schema_id)),
        "{name} inventory scanner"
    );
    value
}

fn expected_inventory_scanner(name: &str, expected_schema_id: &str) -> &'static str {
    match expected_schema_id {
        allow_report::MIGRATE_SCHEMA_ID => allow_report::INVENTORY_SCANNER_POLICY_MIGRATION,
        allow_report::ADD_SCHEMA_ID
        | allow_report::DOCTOR_SCHEMA_ID
        | allow_report::EXPLAIN_SCHEMA_ID
        | allow_report::LIST_SCHEMA_ID
        | allow_report::PROPOSE_SCHEMA_ID
        | allow_report::PRUNE_SCHEMA_ID
        | allow_report::RECEIPT_SCHEMA_ID
        | allow_report::REPORT_SCHEMA_ID
        | allow_report::WORKLIST_SCHEMA_ID => allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
        _ => std::panic::panic_any(format!(
            "{name} expected schema_id {expected_schema_id} has no registered inventory scanner"
        )),
    }
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

fn assert_json_array_eq(value: &Value, field: &str, expected: &[&str], artifact: &str) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{artifact} {field} should be an array"));
    };
    let actual = items
        .iter()
        .map(|item| {
            item.as_str().unwrap_or_else(|| {
                std::panic::panic_any(format!("{artifact} {field} entries should be strings"))
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{artifact} {field}");
}
