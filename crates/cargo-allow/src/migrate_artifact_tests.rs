use super::*;
use allow_core::{AllowEntry, Lifecycle, Selector};
use serde_json::Value;

#[test]
fn render_migrate_summary_json_records_policy_migration_context() {
    let mut cfg = AllowConfig::empty();
    let mut baseline = test_entry("allow-baseline", FindingKind::Panic);
    baseline.classification = "baseline_debt".to_string();
    let mut unsafe_entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    unsafe_entry.evidence = vec!["unsafe-review:docs/evidence/unsafe.json".to_string()];
    cfg.allow.push(baseline);
    cfg.allow.push(unsafe_entry);
    let context = MigrateContext {
        inventory_source: "git_tracked".to_string(),
        source_tree_root: Some("H:/Code/Rust/cargo-allow".to_string()),
        inventory_files: Some(53),
        input_kind: "repo_policy".to_string(),
        input_path: "policy".to_string(),
    };

    let json = render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), true);
    let value = parse_json_artifact("migrate", &json, allow_report::MIGRATE_SCHEMA_ID, "migrate");

    assert_inventory_contract(
        "migrate",
        &value,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(53),
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("policy_migration"),
        "migrate scanner"
    );
    assert_eq!(
        value.pointer("/input/kind").and_then(Value::as_str),
        Some("repo_policy"),
        "migrate input kind"
    );
    assert_eq!(
        value.pointer("/output/path").and_then(Value::as_str),
        Some("policy/allow.toml"),
        "migrate output path"
    );
    assert_eq!(
        value.pointer("/output/force").and_then(Value::as_bool),
        Some(true),
        "migrate force"
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(2),
        "migrate allow entries"
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt")
            .and_then(Value::as_u64),
        Some(1),
        "migrate baseline debt"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(Value::as_u64),
        Some(1),
        "migrate unsafe entries"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(Value::as_u64),
        Some(1),
        "migrate evidence entries"
    );
    assert!(json.contains("policy_migration"));
}

#[test]
fn migrate_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/migrate.schema.json");

    assert!(schema.contains(allow_report::MIGRATE_SCHEMA_ID));
    assert!(schema.contains("\"policy_migration\""));
    assert!(schema.contains("\"input\""));
    assert!(schema.contains("\"output\""));
    assert!(schema.contains("\"allow_entries\""));
    assert!(schema.contains("\"baseline_debt\""));
    assert!(schema.contains("\"unsafe_entries\""));
    assert!(schema.contains("\"entries_with_evidence\""));
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

fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "team".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector::default(),
        last_seen: None,
    }
}
