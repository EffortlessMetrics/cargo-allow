use super::test_support::{test_entry, test_finding};
use super::*;
use allow_core::FindingKind;
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

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::EXPLAIN_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"explain\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"cargo_metadata_not_invoked\""));
    assert!(json.contains("\"repository_code_not_executed\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 47"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"current_status\": \"matched\""));
    assert!(json.contains("\"current_matches\": 1"));
    assert!(json.contains("\"path\": \"tracked.file\""));
    assert!(json.contains("\"source_package\": \"allow-core\""));
    assert!(json.contains("\"status\": \"traceability_only\""));
    assert!(json.contains("\"suggested_actions\": []"));
    assert!(json.contains("\"proof_commands\": []"));
}

#[test]
fn explain_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/explain.schema.json");

    assert!(schema.contains(allow_report::EXPLAIN_SCHEMA_ID));
    assert!(schema.contains("\"allow_entry\""));
    assert!(schema.contains("\"evidence_references\""));
    assert!(schema.contains("\"current_findings\""));
    assert!(schema.contains("\"match_outcomes\""));
    assert!(schema.contains("\"next\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"source_package\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}
