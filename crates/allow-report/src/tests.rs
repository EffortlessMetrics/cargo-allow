use super::*;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
};
use std::path::PathBuf;

#[test]
fn policy_and_finding_json_helpers_render_current_contract() {
    let entry = AllowEntry {
        id: "allow-json".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("crates\\parser\\src\\lib.rs")),
        glob: None,
        owner: "parser".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "generated baseline".to_string(),
        evidence: vec!["test:parser_handles_empty".to_string()],
        links: vec!["adr:docs/adr/0001.md".to_string()],
        occurrence_limit: Some(2),
        lifecycle: Lifecycle {
            created: Some("2026-05-27".to_string()),
            review_after: Some("2026-07-01".to_string()),
            expires: Some("2026-08-02".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("parse".to_string()),
            callee: Some("unwrap".to_string()),
            macro_name: None,
            lint: None,
            symbol: Some("value.unwrap()".to_string()),
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: Some("fnv1a64:test".to_string()),
            line_hint: Some(12),
            glob: None,
        },
        last_seen: Some(LastSeen {
            line: 12,
            column: 9,
        }),
    };
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    identity.container = Some("parse".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates\\parser\\src\\lib.rs"),
        span: Some(Span {
            line: 12,
            column: 9,
        }),
        identity,
        message: "unwrap call".to_string(),
    };

    let entry_json = render_allow_entry_json(&entry, "  ");
    let finding_json = render_explain_finding_json(&finding, "selected", "  ");

    assert!(entry_json.contains("\"id\": \"allow-json\""));
    assert!(entry_json.contains("\"path\": \"crates/parser/src/lib.rs\""));
    assert!(entry_json.contains("\"occurrence_limit\": 2"));
    assert!(entry_json.contains("\"normalized_snippet_hash\": \"fnv1a64:test\""));
    assert!(entry_json.contains("\"line\": 12"));
    assert!(finding_json.contains("\"status\": \"selected\""));
    assert!(finding_json.contains("\"path\": \"crates/parser/src/lib.rs\""));
    assert!(finding_json.contains("\"source_package\": \"parser\""));
    assert!(finding_json.contains("\"container\": \"parse\""));
}

#[test]
fn receipt_exposes_v1_schema_contract() {
    let json = render_receipt_with_context(
        "check",
        &[],
        true,
        ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(42),
            ..ReportContext::default()
        },
    );
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"schema_id\": \"cargo-allow.receipt.v1\""));
    assert!(json.contains("\"failed\": true"));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 42"));
    assert!(json.contains("\"cargo_metadata_not_invoked\""));
    assert!(json.contains("\"cargo_commands_not_invoked\""));
    assert!(json.contains("\"build_output_not_analyzed\""));
    assert!(json.contains("\"macro_token_tree_contents_not_analyzed\""));
    assert!(json.contains("\"missing_required_field\": 0"));
    assert!(json.contains("\"evidence_missing\": 0"));
}
