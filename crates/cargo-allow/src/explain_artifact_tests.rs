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
            .pointer("/summary/broad_scope")
            .and_then(Value::as_bool),
        Some(false),
        "explain broad scope"
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

#[test]
fn explain_entry_json_records_allow_id_proof_command_for_attention_items() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-baseline", FindingKind::Panic);
    entry.family = Some("unwrap".to_string());
    entry.classification = "baseline_debt".to_string();
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
    );

    let json = explain_entry_json(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
        ExplainContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "filesystem_fallback",
                Some("fixtures/source-snapshot"),
                Some(1),
            ),
        },
    );
    let value = parse_json_artifact("explain", &json, allow_report::EXPLAIN_SCHEMA_ID, "explain");

    let proof_commands = value
        .pointer("/next/proof_commands")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("explain proof commands should be an array"));
    assert!(
        proof_commands.iter().any(|command| {
            command.as_str() == Some("cargo-allow worklist --allow-id allow-baseline --format json")
        }),
        "explain proof commands should reopen the durable allow-id queue"
    );
    assert!(
        proof_commands
            .iter()
            .any(|command| command.as_str() == Some("cargo-allow explain allow-baseline")),
        "explain proof commands should keep the direct explain command"
    );
}

#[test]
fn explain_entry_json_routes_broken_evidence_to_repair_queue() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-broken-evidence", FindingKind::NonRustFile);
    entry.evidence = vec!["doc:docs/missing-evidence.md".to_string()];
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );

    let json = explain_entry_json(
        Path::new("target/cargo-allow-test-missing-root"),
        &cfg,
        &entry,
        &[finding],
        ExplainContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "filesystem_fallback",
                Some("fixtures/source-snapshot"),
                Some(1),
            ),
        },
    );
    let value = parse_json_artifact("explain", &json, allow_report::EXPLAIN_SCHEMA_ID, "explain");

    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(Value::as_str),
        Some("local_file_missing"),
        "explain should surface the broken local evidence diagnostic"
    );
    let suggested_actions = value
        .pointer("/next/suggested_actions")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("explain suggested actions should be an array"));
    assert!(
        suggested_actions.iter().any(|action| action.as_str()
            == Some("restore or commit the referenced local evidence artifact")),
        "explain should suggest local evidence repair"
    );
    let proof_commands = value
        .pointer("/next/proof_commands")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("explain proof commands should be an array"));
    assert!(
        proof_commands.iter().any(|command| command.as_str()
            == Some("cargo-allow worklist --item-kind broken_evidence_link --format json")),
        "explain should route broken evidence to the worklist repair queue"
    );
}
