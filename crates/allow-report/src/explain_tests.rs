use super::*;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, LedgerProvenance, Lifecycle, MatchOutcome,
    MatchStatus, Selector, Span, StructuralIdentity,
};
use std::path::PathBuf;

#[test]
fn explain_finding_json_omits_unavailable_metadata() {
    let finding = Finding {
        kind: FindingKind::Panic,
        family: None,
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: StructuralIdentity::new("rust", "panic_macro"),
        message: "panic macro".to_string(),
        ledger: None,
    };

    let json = render_explain_finding_json(&finding, "unmatched", "");

    assert!(!json.contains("\"family\":"));
    assert!(!json.contains("\"source_package\":"));
    assert!(json.contains("\"line\": null"));
    assert!(json.contains("\"column\": null"));
}

#[test]
fn explain_finding_json_preserves_ledger_provenance() {
    let finding = Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: StructuralIdentity::new("rust", "unsafe_block"),
        message: "unsafe block".to_string(),
        ledger: Some(LedgerProvenance {
            ledger_id: "source-policy".to_string(),
            ledger_path: "policy/allow.toml".to_string(),
            lane: "source-exception".to_string(),
            mode: "blocking".to_string(),
            role: "canonical".to_string(),
        }),
    };

    let json = render_explain_finding_json(&finding, "matched", "");

    for expected in [
        "\"family\": \"unsafe_block\"",
        "\"ledger_id\": \"source-policy\"",
        "\"ledger_path\": \"policy/allow.toml\"",
        "\"lane\": \"source-exception\"",
        "\"mode\": \"blocking\"",
        "\"role\": \"canonical\"",
    ] {
        assert!(json.contains(expected), "{expected}");
    }
}

#[test]
fn explain_projects_expired_matched_entry_lifecycle_status() {
    let entry = AllowEntry {
        id: "allow-explain-expired".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "owner".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "test policy entry".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2020-01-01".to_string()),
        },
        selector: Selector::default(),
        last_seen: None,
    };
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some(entry.id.clone()),
        candidate_ids: Vec::new(),
        finding_index: None,
        message: "matched".to_string(),
        score: 100,
    }];
    let report = ExplainReport {
        inventory: InventoryContext::default(),
        entry: &entry,
        selector_precision: 0,
        broad_scope: false,
        current_findings: &[],
        match_outcomes: &outcomes,
        evidence_references: &[],
        link_references: &[],
        suggested_actions: &[],
        proof_commands: &[],
    };

    let human = render_explain_human(report);
    let json = render_explain_json(report);

    assert!(human.contains("current_status: expired"));
    assert!(json.contains("\"current_status\": \"expired\""));
}

#[test]
fn explain_projects_review_due_matched_entry_lifecycle_status() {
    let entry = AllowEntry {
        id: "allow-explain-review-due".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "owner".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "test policy entry".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: Some("2020-01-01".to_string()),
            expires: None,
        },
        selector: Selector::default(),
        last_seen: None,
    };
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some(entry.id.clone()),
        candidate_ids: Vec::new(),
        finding_index: None,
        message: "matched".to_string(),
        score: 100,
    }];
    let report = ExplainReport {
        inventory: InventoryContext::default(),
        entry: &entry,
        selector_precision: 0,
        broad_scope: false,
        current_findings: &[],
        match_outcomes: &outcomes,
        evidence_references: &[],
        link_references: &[],
        suggested_actions: &[],
        proof_commands: &[],
    };

    let human = render_explain_human(report);
    let json = render_explain_json(report);

    assert!(human.contains("current_status: review_due"));
    assert!(json.contains("\"current_status\": \"review_due\""));
}

#[test]
fn explain_json_renderer_records_context_and_current_status() {
    let entry = AllowEntry {
        id: "allow-explain-json".to_string(),
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: Some(PathBuf::from("src\\ffi.rs")),
        glob: None,
        owner: "runtime".to_string(),
        classification: "ffi_boundary".to_string(),
        reason: "FFI pointer boundary requires unsafe.".to_string(),
        evidence: vec!["doc:docs/safety/ffi.md".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-27".to_string()),
            review_after: Some("2026-11-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("unsafe_block".to_string()),
            container: Some("read_byte".to_string()),
            callee: None,
            macro_name: None,
            lint: None,
            symbol: None,
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: Some("fnv1a64:unsafe".to_string()),
            line_hint: Some(9),
            glob: None,
        },
        last_seen: Some(LastSeen { line: 9, column: 5 }),
    };
    let mut identity = StructuralIdentity::new("rust", "unsafe_block");
    identity.crate_name = Some("runtime".to_string());
    identity.container = Some("read_byte".to_string());
    let finding = Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: PathBuf::from("src\\ffi.rs"),
        span: Some(Span { line: 9, column: 5 }),
        identity,
        message: "unsafe block".to_string(),
        ledger: None,
    };
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::EvidenceMissing,
        allow_id: Some("allow-explain-json".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unsafe entry has missing evidence".to_string(),
        score: 9,
    }];
    let evidence_references = vec![EvidenceReference {
        raw: "doc:docs/safety/ffi.md",
        prefix: Some("doc"),
        target: Some("docs/safety/ffi.md"),
        status: "local_file_missing",
        category: "missing",
        message: "local evidence file is missing",
    }];
    let link_references = Vec::new();
    let suggested_actions = vec!["add missing evidence".to_string()];
    let proof_commands = vec!["cargo-allow check --kind unsafe".to_string()];

    let report = ExplainReport {
        inventory: InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        entry: &entry,
        selector_precision: 42,
        broad_scope: true,
        current_findings: &[finding],
        match_outcomes: &outcomes,
        evidence_references: &evidence_references,
        link_references: &link_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    };

    let json = render_explain_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.explain.v1\""));
    assert!(json.contains("\"command\": \"explain\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"id\": \"allow-explain-json\""));
    assert!(json.contains("\"current_status\": \"evidence_missing\""));
    assert!(json.contains("\"current_matches\": 1"));
    assert!(json.contains("\"match_outcomes\": 1"));
    assert!(json.contains("\"selector_precision\": 42"));
    assert!(json.contains("\"broad_scope\": true"));
    assert!(json.contains("\"raw\": \"doc:docs/safety/ffi.md\""));
    assert!(json.contains("\"target\": \"docs/safety/ffi.md\""));
    assert!(json.contains("\"status\": \"local_file_missing\""));
    assert!(json.contains("\"path\": \"src/ffi.rs\""));
    assert!(json.contains("\"source_package\": \"runtime\""));
    assert!(json.contains("\"score\": 9"));
    assert!(json.contains("\"add missing evidence\""));
    assert!(json.contains("\"cargo-allow check --kind unsafe\""));
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.explain.v1",
  "tool": "cargo-allow",
  "command": "explain",
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 76
  }},
  "allow_entry": {{
    "id": "allow-explain-json",
    "kind": "unsafe",
    "family": "unsafe_block",
    "scope": "src/ffi.rs",
    "path": "src/ffi.rs",
    "glob": null,
    "owner": "runtime",
    "classification": "ffi_boundary",
    "reason": "FFI pointer boundary requires unsafe.",
    "evidence": ["doc:docs/safety/ffi.md"],
    "links": [],
    "occurrence_limit": null,
    "lifecycle": {{
      "created": "2026-05-27",
      "review_after": "2026-11-01"
    }},
    "selector": {{
      "ast_kind": "unsafe_block",
      "container": "read_byte",
      "callee": null,
      "macro_name": null,
      "lint": null,
      "symbol": null,
      "receiver_fingerprint": null,
      "target_fingerprint": null,
      "normalized_snippet_hash": "fnv1a64:unsafe",
      "line_hint": 9,
      "glob": null
    }},
    "last_seen": {{
      "line": 9,
      "column": 5
    }}
  }},
  "summary": {{
    "current_status": "evidence_missing",
    "current_matches": 1,
    "match_outcomes": 1,
    "selector_precision": 42,
    "broad_scope": true
  }},
  "evidence_references": [
    {{
      "raw": "doc:docs/safety/ffi.md",
      "prefix": "doc",
      "target": "docs/safety/ffi.md",
      "status": "local_file_missing",
      "category": "missing",
      "message": "local evidence file is missing"
    }}
  ],
  "current_findings": [
    {{
      "status": "evidence_missing",
      "kind": "unsafe",
      "family": "unsafe_block",
      "path": "src/ffi.rs",
      "line": 9,
      "column": 5,
      "source_package": "runtime",
      "identity": {{
        "language": "rust",
        "crate_name": "runtime",
        "module": null,
        "container": "read_byte",
        "ast_kind": "unsafe_block",
        "symbol": null,
        "callee": null,
        "macro_name": null,
        "lint": null,
        "receiver_fingerprint": null,
        "target_fingerprint": null,
        "normalized_snippet_hash": null,
        "line_hint": null,
        "column_hint": null
      }},
      "message": "unsafe block"
    }}
  ],
  "match_outcomes": [
    {{
      "status": "evidence_missing",
      "allow_id": "allow-explain-json",
      "candidate_ids": [],
      "finding_index": 0,
      "score": 9,
      "message": "unsafe entry has missing evidence"
    }}
  ],
  "next": {{
    "suggested_actions": ["add missing evidence"],
    "proof_commands": ["cargo-allow check --kind unsafe"]
  }}
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);

    let text = render_explain_human(report);

    assert!(text.contains("allow-explain-json"));
    assert!(text.contains("kind: unsafe.unsafe_block"));
    assert!(text.contains("scope: src/ffi.rs"));
    assert!(text.contains("owner: runtime"));
    assert!(text.contains("classification: ffi_boundary"));
    assert!(text.contains("selector_precision: 42"));
    assert!(text.contains("broad_scope: true"));
    assert!(text.contains("evidence diagnostics:"));
    assert!(text.contains(
        "- missing: doc:docs/safety/ffi.md (status=local_file_missing, prefix=doc, target=docs/safety/ffi.md)"
    ));
    assert!(text.contains("  message: local evidence file is missing"));
    assert!(text.contains("current_status: evidence_missing"));
    assert!(text.contains("current_matches: 1"));
    assert!(
        text.contains("- evidence_missing: src/ffi.rs:9:5 (unsafe_block, source_package=runtime)")
    );
    assert!(text.contains("- evidence_missing: unsafe entry has missing evidence"));
    assert!(text.contains("- action: add missing evidence"));
    assert!(text.contains("- proof: cargo-allow check --kind unsafe"));
}
