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
