use super::*;
use crate::MutationReceipt;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
};
use serde_json::Value;
use std::path::PathBuf;

fn sample_mutation_receipt() -> MutationReceipt<'static> {
    MutationReceipt {
        operation: "add",
        tool_version: "0.1.9",
        repo_root: Some("H:/Code/Rust/cargo-allow"),
        config_source: Some("policy/allow.toml"),
        ledger_ids: Vec::new(),
        changed_allow_ids: vec!["allow-add-json"],
        before_fingerprints: vec![None],
        after_fingerprints: vec![Some(
            "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        )],
        result: "written",
        next_commands: Vec::new(),
    }
}

#[test]
fn add_json_renderer_records_entry_and_selected_finding() -> Result<(), String> {
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
        ledger: None,
    };

    let json = render_add_json(AddReport::new(
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(52)),
        &entry,
        &finding,
        Some("policy/allow.proposed.toml"),
        true,
        sample_mutation_receipt(),
    ));

    assert!(json.contains("\"schema_id\": \"cargo-allow.add.v1\""));
    assert!(json.contains("\"command\": \"add\""));
    assert!(json.contains("\"mutation_receipt\": {"));
    assert!(json.contains("\"schema_id\": \"cargo-allow.mutation-receipt.v1\""));
    assert!(json.contains("\"changed_allow_ids\": [\"allow-add-json\"]"));
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
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.add.v1",
  "tool": "cargo-allow",
  "command": "add",
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 52
  }},
  "mutation_receipt": {{
      "schema_id": "cargo-allow.mutation-receipt.v1",
      "operation": "add",
      "tool_version": "0.1.9",
      "repo_root": "H:/Code/Rust/cargo-allow",
      "config_source": "policy/allow.toml",
      "ledger_ids": [],
      "changed_allow_ids": ["allow-add-json"],
      "before_fingerprints": [null],
      "after_fingerprints": ["sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"],
      "result": "written",
      "next_commands": [],
      "claim_boundary": "Provenance envelope only: records what changed and how to verify it. Does not itself validate entry correctness, authorize merge, or change command semantics (GOAL-0004 PR 5, CARGO-ALLOW-SPEC-0008)."
    }},
  "options": {{
    "policy_output": "policy/allow.proposed.toml",
    "force": true
  }},
  "summary": {{
    "entry_id": "allow-add-json",
    "selected_finding": "src/lib.rs:42:13",
    "human_review_required": true
  }},
  "allow_entry": {{
    "id": "allow-add-json",
    "kind": "panic",
    "family": "unwrap",
    "path": "src/lib.rs",
    "glob": null,
    "owner": "parser",
    "classification": "reviewed_exception",
    "reason": "Parser validates input before unwrapping.",
    "review_after": "2026-11-01",
    "expires": "2027-01-01",
    "evidence_count": 1,
    "selector": {{
        "ast_kind": "method_call",
        "container": "parse_span",
        "callee": "unwrap",
        "macro_name": null,
        "lint": null,
        "symbol": "value.unwrap()",
        "receiver_fingerprint": null,
        "target_fingerprint": null,
        "normalized_snippet_hash": "fnv1a64:add",
        "line_hint": 42,
        "glob": null
      }},
    "last_seen": {{
        "line": 42,
        "column": 13
      }}
  }},
  "selected_finding":     {{
      "status": "selected",
      "kind": "panic",
      "family": "unwrap",
      "path": "src/lib.rs",
      "line": 42,
      "column": 13,
      "source_package": "parser",
      "identity": {{
        "language": "rust",
        "crate_name": "parser",
        "module": null,
        "container": "parse_span",
        "ast_kind": "method_call",
        "symbol": null,
        "callee": "unwrap",
        "macro_name": null,
        "lint": null,
        "receiver_fingerprint": null,
        "target_fingerprint": null,
        "normalized_snippet_hash": null,
        "line_hint": null,
        "column_hint": null
      }},
      "message": "unwrap call"
    }}
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);

    let text = render_add_human(AddReport::new(
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(52)),
        &entry,
        &finding,
        Some("policy/allow.proposed.toml"),
        false,
        sample_mutation_receipt(),
    ));

    assert!(text.contains("cargo-allow add summary"));
    assert!(
        text.contains("inventory: source_tree/source_syntax via git_tracked; files scanned: 52")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("id: allow-add-json"));
    assert!(text.contains("kind: panic"));
    assert!(text.contains("family: unwrap"));
    assert!(text.contains("matched finding: src/lib.rs:42:13"));
    assert!(text.contains("output: policy/allow.proposed.toml"));
    assert!(text.contains("requires human review"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));

    let styled = render_add_human_styled(
        AddReport::new(
            InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(52),
            ),
            &entry,
            &finding,
            Some("policy/allow.proposed.toml"),
            false,
            sample_mutation_receipt(),
        ),
        Style::ANSI,
    );
    assert!(styled.contains("requires human \u{1b}[33mreview\u{1b}[0m before merge"));
    assert_eq!(styled.matches('\u{1b}').count(), 2);

    let mut sparse_entry = entry.clone();
    sparse_entry.family = None;
    sparse_entry.path = None;
    sparse_entry.glob = Some("src/**/*.rs".to_string());
    sparse_entry.lifecycle = Lifecycle::empty();
    sparse_entry.last_seen = None;
    let sparse_json = render_add_json(AddReport::new(
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(52)),
        &sparse_entry,
        &finding,
        None,
        false,
        sample_mutation_receipt(),
    ));
    let sparse_value: Value = serde_json::from_str(&sparse_json)
        .map_err(|error| format!("sparse add report should render valid JSON: {error}"))?;
    let sparse_allow_entry = sparse_value
        .get("allow_entry")
        .ok_or_else(|| "sparse add report should include allow_entry".to_string())?;
    for field in ["family", "review_after", "expires"] {
        if sparse_allow_entry.get(field).is_some() {
            return Err(format!("sparse add report should omit {field}"));
        }
    }
    if sparse_allow_entry.get("path") != Some(&Value::Null)
        || sparse_allow_entry.get("last_seen") != Some(&Value::Null)
        || sparse_allow_entry.get("glob").and_then(Value::as_str) != Some("src/**/*.rs")
    {
        return Err(
            "add report should retain nullable and selector relationship fields".to_string(),
        );
    }

    Ok(())
}
