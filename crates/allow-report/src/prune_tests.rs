use super::*;

#[test]
fn prune_json_renderer_records_mode_context_and_candidates() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale",
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        reason: "stale baseline entry",
    }];

    let json = render_prune_json(
        &candidates,
        PruneModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(49)),
    );

    assert!(json.contains("\"schema_id\": \"cargo-allow.prune.v1\""));
    assert!(json.contains("\"command\": \"prune\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 49"));
    assert!(json.contains("\"dry_run\": true"));
    assert!(json.contains("\"write_requested\": false"));
    assert!(json.contains("\"explicit_dry_run\": true"));
    assert!(json.contains("\"written_path\": null"));
    assert!(json.contains("\"stale_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-stale\""));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.prune.v1",
  "tool": "cargo-allow",
  "command": "prune",
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 49
  }},
  "mode": {{
    "dry_run": true,
    "write_requested": false,
    "explicit_dry_run": true,
    "written_path": null
  }},
  "summary": {{
    "stale_entries": 1
  }},
  "stale_entries": [
    {{
      "id": "allow-stale",
      "kind": "panic",
      "family": "unwrap",
      "owner": "parser",
      "classification": "baseline_debt",
      "scope": "crates/parser/src/lib.rs",
      "reason": "stale baseline entry"
    }}
  ]
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);
}

#[test]
fn prune_human_renderer_records_mode_and_candidates() {
    let candidates = vec![PruneCandidate {
        id: "allow|stale",
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        reason: "old | baseline entry",
    }];

    let text = render_prune_human(
        &candidates,
        PruneModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
        },
    );

    assert!(text.contains("mode: dry-run"));
    assert!(text.contains("requested: --dry-run"));
    assert!(text.contains("stale entries: 1"));
    assert!(text.contains("allow\\|stale"));
    assert!(text.contains("old \\| baseline entry"));
    assert!(text.contains("No files were changed"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}

#[test]
fn prune_human_renderer_records_inventory_context() {
    let text = render_prune_human_with_context(
        &[],
        PruneModeContext {
            explicit_dry_run: false,
            write_requested: false,
            written_path: None,
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(49)),
    );

    assert!(
        text.contains("Inventory: source_tree/source_syntax via git_tracked; files scanned: 49")
    );
    assert!(text.contains("Source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("No stale allow entries found."));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}
