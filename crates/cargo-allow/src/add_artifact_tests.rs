use super::test_support::test_finding_at_line;
use super::*;
use serde_json::Value;

#[test]
fn render_add_summary_json_records_entry_and_selected_finding() {
    let mut finding = test_finding_at_line(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
        42,
    );
    finding.identity.crate_name = Some("parser".to_string());
    finding.identity.container = Some("parse_span".to_string());
    finding.identity.callee = Some("unwrap".to_string());
    let mut entry = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-0101".to_string(),
        owner: "parser".to_string(),
        classification: "validated_invariant".to_string(),
        reason: "Parser validates the span before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_span".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: Some("2027-01-01".to_string()),
    });
    entry.selector.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

    let json = render_add_summary_json(
        &entry,
        &finding,
        Some(Path::new("policy/allow.proposed.toml")),
        true,
        AddContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(52),
        },
    );
    let value = parse_json_artifact("add", &json, allow_report::ADD_SCHEMA_ID, "add");

    assert_inventory_contract(
        "add",
        &value,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(52),
    );
    assert_eq!(
        value
            .pointer("/options/policy_output")
            .and_then(Value::as_str),
        Some("policy/allow.proposed.toml"),
        "add policy output"
    );
    assert_eq!(
        value.pointer("/options/force").and_then(Value::as_bool),
        Some(true),
        "add force"
    );
    assert_eq!(
        value.pointer("/summary/entry_id").and_then(Value::as_str),
        Some("allow-0101"),
        "add summary entry id"
    );
    assert_eq!(
        value
            .pointer("/summary/human_review_required")
            .and_then(Value::as_bool),
        Some(true),
        "add human_review_required"
    );
    assert_eq!(
        value.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-0101"),
        "add allow id"
    );
    assert_eq!(
        value
            .pointer("/allow_entry/evidence_count")
            .and_then(Value::as_u64),
        Some(1),
        "add evidence count"
    );
    assert_eq!(
        value
            .pointer("/selected_finding/source_package")
            .and_then(Value::as_str),
        Some("parser"),
        "add selected finding source package"
    );
}

#[test]
fn add_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/add.schema.json");

    assert!(schema.contains(allow_report::ADD_SCHEMA_ID));
    assert!(schema.contains("\"options\""));
    assert!(schema.contains("\"policy_output\""));
    assert!(schema.contains("\"allow_entry\""));
    assert!(schema.contains("\"selected_finding\""));
    assert!(schema.contains("\"human_review_required\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}

fn parse_json_artifact(
    name: &str,
    json: &str,
    expected_schema: &str,
    expected_command: &str,
) -> Value {
    let value: Value = serde_json::from_str(json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("{name} json: {err}\n{json}")));
    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some(expected_schema),
        "{name} schema id"
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some(expected_command),
        "{name} command"
    );
    value
}

fn assert_inventory_contract(
    name: &str,
    value: &Value,
    expected_source: &str,
    expected_root: Option<&str>,
    expected_files: Option<u64>,
) {
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree"),
        "{name} inventory scope"
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("source_syntax"),
        "{name} inventory scanner"
    );
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
        "{name} inventory files"
    );
}
