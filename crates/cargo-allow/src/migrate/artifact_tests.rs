use super::*;
use crate::artifact_contract_support::{assert_inventory_contract, parse_json_artifact};
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
