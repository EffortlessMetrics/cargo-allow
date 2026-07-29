use super::*;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
};
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn policy_and_finding_json_helpers_render_current_contract() -> Result<(), String> {
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
        ledger: None,
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

    let mut sparse_entry = entry.clone();
    sparse_entry.family = None;
    sparse_entry.path = None;
    sparse_entry.glob = Some("src/**/*.rs".to_string());
    sparse_entry.occurrence_limit = None;
    sparse_entry.lifecycle = Lifecycle::empty();
    sparse_entry.last_seen = None;
    let sparse_value: Value =
        serde_json::from_str(&render_allow_entry_json(&sparse_entry, "  "))
            .map_err(|error| format!("sparse allow entry should render valid JSON: {error}"))?;
    for field in ["family", "created", "review_after", "expires"] {
        if sparse_value.get(field).is_some() {
            return Err(format!("sparse allow entry should omit {field}"));
        }
    }
    if sparse_value.get("lifecycle") != Some(&Value::Object(Default::default())) {
        return Err("sparse allow entry should retain an empty lifecycle object".to_string());
    }
    if sparse_value.get("path") != Some(&Value::Null)
        || sparse_value.get("occurrence_limit") != Some(&Value::Null)
        || sparse_value.get("last_seen") != Some(&Value::Null)
    {
        return Err("relationship and matching-state nulls should remain present".to_string());
    }

    Ok(())
}
