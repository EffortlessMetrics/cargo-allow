use super::*;

#[test]
fn list_json_renderer_records_filters_context_and_rows() {
    let rows = vec![ListRow {
        id: "allow-json",
        status: "baseline_debt",
        matches: 1,
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        source_package: Some("parser"),
        evidence_count: 2,
        selector_precision: 42,
        broad_scope: true,
        review_after: Some("2026-07-01"),
        expires: None,
        reason: "generated baseline",
    }];

    let json = render_list_json(
        &rows,
        ListFilters {
            kind: Some("panic"),
            family: Some("unwrap"),
            owner: Some("parser"),
            allow_id: Some("allow-json"),
            baseline_debt: true,
            ..ListFilters::default()
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(46)),
    );

    assert!(json.contains("\"schema_id\": \"cargo-allow.list.v1\""));
    assert!(json.contains("\"command\": \"list\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 46"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"owner\": \"parser\""));
    assert!(json.contains("\"allow_id\": \"allow-json\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"allow_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"selector_precision\": 42"));
    assert!(json.contains("\"broad_scope\": true"));
    assert!(json.contains("\"review_after\": \"2026-07-01\""));
    assert!(json.contains("\"expires\": null"));
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.list.v1",
  "tool": "cargo-allow",
  "command": "list",
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 46
  }},
  "filters": {{
    "kind": "panic",
    "family": "unwrap",
    "owner": "parser",
    "classification": null,
    "path": null,
    "source_package": null,
    "allow_id": "allow-json",
    "status": null,
    "expired": false,
    "review_due": false,
    "stale": false,
    "baseline_debt": true,
    "broad_scope": false,
    "missing_evidence": false
  }},
  "summary": {{
    "allow_entries": 1
  }},
  "allow_entries": [
    {{
      "id": "allow-json",
      "status": "baseline_debt",
      "matches": 1,
      "kind": "panic",
      "family": "unwrap",
      "owner": "parser",
      "classification": "baseline_debt",
      "scope": "crates/parser/src/lib.rs",
      "source_package": "parser",
      "evidence_count": 2,
      "selector_precision": 42,
      "broad_scope": true,
      "review_after": "2026-07-01",
      "expires": null,
      "reason": "generated baseline"
    }}
  ]
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);

    let text = render_list_human(&rows);

    assert!(text.starts_with("id\tstatus\tmatches\tkind\tfamily"));
    assert!(text.contains(
            "allow-json\tbaseline_debt\t1\tpanic\tunwrap\tparser\tbaseline_debt\tcrates/parser/src/lib.rs\tparser\t2\t42\ttrue\t2026-07-01\t-\tgenerated baseline"
        ));
}
