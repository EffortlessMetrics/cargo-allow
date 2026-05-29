use super::test_support::{test_entry, test_finding};
use super::*;
use crate::artifact_contract_support::{assert_inventory_contract, parse_json_artifact};
use allow_core::FindingKind;
use serde_json::Value;
use std::path::Path;

#[test]
fn explain_entry_json_records_context_and_live_status() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-json", FindingKind::NonRustFile);
    entry.family = Some("documentation".to_string());
    entry.evidence = vec!["test:explain_fixture".to_string()];
    entry.lifecycle.created = Some("2026-05-27".to_string());
    entry.lifecycle.review_after = Some("2026-11-01".to_string());
    cfg.allow.push(entry.clone());
    let mut finding = test_finding(
        FindingKind::NonRustFile,
        Some("documentation"),
        "tracked.file",
        "tracked_file",
    );
    finding.identity.crate_name = Some("allow-core".to_string());

    let json = explain_entry_json(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
        ExplainContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(47),
            ),
        },
    );
    let value = parse_json_artifact("explain", &json, allow_report::EXPLAIN_SCHEMA_ID, "explain");

    assert_inventory_contract(
        "explain",
        &value,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(47),
    );
    assert_eq!(
        value.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-json"),
        "explain allow id"
    );
    assert_eq!(
        value
            .pointer("/summary/current_status")
            .and_then(Value::as_str),
        Some("matched"),
        "explain current status"
    );
    assert_eq!(
        value
            .pointer("/summary/current_matches")
            .and_then(Value::as_u64),
        Some(1),
        "explain current match count"
    );
    assert_eq!(
        value
            .pointer("/current_findings/0/path")
            .and_then(Value::as_str),
        Some("tracked.file"),
        "explain current finding path"
    );
    assert_eq!(
        value
            .pointer("/current_findings/0/source_package")
            .and_then(Value::as_str),
        Some("allow-core"),
        "explain current finding source package"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(Value::as_str),
        Some("traceability_only"),
        "explain evidence status"
    );
    assert_eq!(
        value
            .pointer("/next/suggested_actions")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "explain suggested actions"
    );
    assert_eq!(
        value
            .pointer("/next/proof_commands")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "explain proof commands"
    );
}
