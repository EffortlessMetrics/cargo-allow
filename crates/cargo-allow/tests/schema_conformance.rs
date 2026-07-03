//! Lightweight schema conformance tests (#1957).

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn load_schema(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas")
        .join(format!("{name}.schema.json"));
    let text = fs::read_to_string(&path).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn declared_properties(schema: &Value) -> BTreeSet<String> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|props| props.keys().cloned().collect())
        .unwrap_or_default()
}

const SCHEMA_NAMES: &[&str] = &[
    "add",
    "doctor",
    "explain",
    "list",
    "migrate",
    "propose",
    "prune",
    "receipt",
    "refresh",
    "report",
    "spec-system",
    "worklist",
];

#[test]
fn every_schema_has_additional_properties_false_at_root() {
    for &name in SCHEMA_NAMES {
        let schema = load_schema(name);
        let additional = schema.get("additionalProperties").and_then(Value::as_bool);
        assert_eq!(additional, Some(false), "{name} additionalProperties");
    }
}

#[test]
fn every_schema_declares_schema_id_and_schema_version() {
    for &name in SCHEMA_NAMES {
        let schema = load_schema(name);
        let props = declared_properties(&schema);
        assert!(props.contains("schema_id"), "{name} schema_id");
        assert!(props.contains("schema_version"), "{name} schema_version");
    }
}

#[test]
fn receipt_schema_declares_provenance_fields() {
    let schema = load_schema("receipt");
    let props = declared_properties(&schema);
    assert!(props.contains("mode"), "receipt mode");
    assert!(props.contains("tool_version"), "receipt tool_version");
}

#[test]
fn list_schema_filters_status_enum_includes_baseline_debt() {
    // #1963: the list CLI accepts `--status baseline_debt` and the renderer
    // emits it into `filters.status`, so the schema's filters.status enum must
    // include `baseline_debt` (and match the row-level status enum). Pin both
    // so the contract cannot regress.
    let schema = load_schema("list");
    let filters_status_enum = schema
        .pointer("/properties/filters/properties/status/enum")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("list schema filters.status enum missing"));
    let values: Vec<&str> = filters_status_enum
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        values.contains(&"baseline_debt"),
        "list filters.status enum must include baseline_debt: {values:?}"
    );

    let row_status_enum = schema
        .pointer("/$defs/allow_entry/properties/status/enum")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("list schema allow_entry status enum missing"));
    let row_values: Vec<&str> = row_status_enum.iter().filter_map(Value::as_str).collect();
    assert!(
        row_values.contains(&"baseline_debt"),
        "list allow_entries[].status enum must include baseline_debt: {row_values:?}"
    );
}
