use super::*;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
};
use std::path::PathBuf;

#[test]
fn add_json_renderer_records_entry_and_selected_finding() {
    let entry = AllowEntry {
        id: "allow-add-json".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src\\lib.rs")),
        glob: None,
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates input before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_input".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-27".to_string()),
            review_after: Some("2026-11-01".to_string()),
            expires: Some("2027-01-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("parse_span".to_string()),
            callee: Some("unwrap".to_string()),
            macro_name: None,
            lint: None,
            symbol: Some("value.unwrap()".to_string()),
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: Some("fnv1a64:add".to_string()),
            line_hint: Some(42),
            glob: None,
        },
        last_seen: Some(LastSeen {
            line: 42,
            column: 13,
        }),
    };
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    identity.container = Some("parse_span".to_string());
    identity.callee = Some("unwrap".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src\\lib.rs"),
        span: Some(Span {
            line: 42,
            column: 13,
        }),
        identity,
        message: "unwrap call".to_string(),
    };

    let json = render_add_json(AddReport::new(
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(52)),
        &entry,
        &finding,
        Some("policy/allow.proposed.toml"),
        true,
    ));

    assert!(json.contains("\"schema_id\": \"cargo-allow.add.v1\""));
    assert!(json.contains("\"command\": \"add\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 52"));
    assert!(json.contains("\"policy_output\": \"policy/allow.proposed.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"entry_id\": \"allow-add-json\""));
    assert!(json.contains("\"selected_finding\": \"src/lib.rs:42:13\""));
    assert!(json.contains("\"human_review_required\": true"));
    assert!(json.contains("\"id\": \"allow-add-json\""));
    assert!(json.contains("\"path\": \"src/lib.rs\""));
    assert!(json.contains("\"review_after\": \"2026-11-01\""));
    assert!(json.contains("\"expires\": \"2027-01-01\""));
    assert!(json.contains("\"evidence_count\": 1"));
    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"normalized_snippet_hash\": \"fnv1a64:add\""));

    let text = render_add_human(AddReport::new(
        InventoryContext::source_syntax("git_tracked", None, None),
        &entry,
        &finding,
        Some("policy/allow.proposed.toml"),
        false,
    ));

    assert!(text.contains("cargo-allow add summary"));
    assert!(text.contains("id: allow-add-json"));
    assert!(text.contains("kind: panic"));
    assert!(text.contains("family: unwrap"));
    assert!(text.contains("matched finding: src/lib.rs:42:13"));
    assert!(text.contains("output: policy/allow.proposed.toml"));
    assert!(text.contains("requires human review"));
}
