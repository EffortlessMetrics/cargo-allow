use super::*;
use crate::worklist_human::render_worklist_human_styled;

#[test]
fn worklist_json_renderer_records_filters_summary_and_items() {
    let suggested_actions = vec!["review stale allow".to_string()];
    let proof_commands = vec!["cargo-allow check --mode no-new".to_string()];
    let candidate_ids = vec!["allow-0001".to_string(), "allow-0002".to_string()];
    let items = vec![WorklistItem {
        id: "work-0001",
        kind: "stale_allow",
        exception_kind: Some("panic"),
        family: Some("unwrap"),
        owner: Some("parser"),
        classification: Some("baseline_debt"),
        reason: Some("generated baseline"),
        created: Some("2026-05-27"),
        review_after: Some("2026-07-01"),
        expires: Some("2026-08-02"),
        evidence_count: Some(1),
        selector_precision: Some(42),
        risk: "high",
        difficulty: "small",
        status: "stale",
        allow_id: Some("allow-0001"),
        candidate_ids: &candidate_ids,
        finding_index: None,
        path: Some("crates/parser/src/lib.rs"),
        line: None,
        column: None,
        evidence_reference: None,
        source_package: Some("parser"),
        message: "stale allow",
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }];

    let json = render_worklist_json(
        &items,
        WorklistFilters {
            kind: Some("panic"),
            item_kind: Some("stale_allow"),
            risk: Some("high"),
            baseline_debt: true,
            missing_evidence: true,
            ..WorklistFilters::default()
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(47)),
    );

    assert!(json.contains("\"schema_id\": \"cargo-allow.worklist.v1\""));
    assert!(json.contains("\"command\": \"worklist\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 47"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"item_kind\": \"stale_allow\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"missing_evidence\": true"));
    assert!(json.contains("\"broken_evidence\": false"));
    assert!(json.contains("\"weak_evidence\": false"));
    assert!(json.contains("\"work_items\": 1"));
    assert!(json.contains("\"high\": 1"));
    assert!(json.contains("\"small_difficulty\": 1"));
    assert!(json.contains("\"item_kinds\": {"));
    assert!(json.contains("\"stale_allow\": 1"));
    assert!(json.contains("\"id\": \"work-0001\""));
    assert!(json.contains("\"exception_kind\": \"panic\""));
    assert!(json.contains("\"evidence_count\": 1"));
    assert!(json.contains("\"selector_precision\": 42"));
    assert!(json.contains("\"finding_index\": null"));
    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"suggested_actions\": [\"review stale allow\"]"));
    assert!(json.contains("\"proof_commands\": [\"cargo-allow check --mode no-new\"]"));
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.worklist.v1",
  "tool": "cargo-allow",
  "command": "worklist",
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 47
  }},
  "filters": {{
    "kind": "panic",
    "family": null,
    "item_kind": "stale_allow",
    "status": null,
    "allow_id": null,
    "path": null,
    "source_package": null,
    "owner": null,
    "classification": null,
    "baseline_debt": true,
    "broad_scope": false,
    "risk": "high",
    "difficulty": null,
    "missing_evidence": true,
    "broken_evidence": false,
    "weak_evidence": false
  }},
  "summary": {{
    "work_items": 1,
    "high": 1,
    "medium": 0,
    "low": 0,
    "small_difficulty": 1,
    "medium_difficulty": 0,
    "item_kinds": {{
      "stale_allow": 1
    }}
  }},
  "work_items": [
    {{
      "id": "work-0001",
      "kind": "stale_allow",
      "exception_kind": "panic",
      "family": "unwrap",
      "owner": "parser",
      "classification": "baseline_debt",
      "reason": "generated baseline",
      "created": "2026-05-27",
      "review_after": "2026-07-01",
      "expires": "2026-08-02",
      "evidence_count": 1,
      "selector_precision": 42,
      "risk": "high",
      "difficulty": "small",
      "status": "stale",
      "allow_id": "allow-0001",
      "candidate_ids": ["allow-0001", "allow-0002"],
      "finding_index": null,
      "path": "crates/parser/src/lib.rs",
      "source_package": "parser",
      "message": "stale allow",
      "suggested_actions": ["review stale allow"],
      "proof_commands": ["cargo-allow check --mode no-new"],
      "ledger_id": null,
      "ledger_path": null,
      "lane": null,
      "mode": null,
      "role": null
    }}
  ]
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);

    let text = render_worklist_human(
        &items,
        WorklistFilters {
            kind: Some("panic"),
            item_kind: Some("stale_allow"),
            risk: Some("high"),
            baseline_debt: true,
            missing_evidence: true,
            ..WorklistFilters::default()
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(47)),
    );

    assert!(text.contains("cargo-allow worklist"));
    assert!(
        text.contains("Inventory: source_tree/source_syntax via git_tracked; files scanned: 47")
    );
    assert!(text.contains("Source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains(
            "Filters: kind=panic, item_kind=stale_allow, baseline_debt=true, risk=high, missing_evidence=true"
        ));
    assert!(text.contains("Work items: 1"));
    assert!(text.contains("Queue kinds:"));
    assert!(text.contains("  stale_allow"));
    assert!(text.contains("work-0001 (high, small) stale_allow"));
    assert!(text.contains("  path: crates/parser/src/lib.rs"));
    assert!(text.contains("  source package: parser"));
    assert!(text.contains("  exception: panic.unwrap"));
    assert!(text.contains("  selector_precision: 42"));
    assert!(text.contains("  action: review stale allow"));
    assert!(text.contains("  proof: cargo-allow check --mode no-new"));
    assert!(text.contains("external evidence tools"));
}

#[test]
fn worklist_json_renderer_includes_optional_evidence_reference() {
    let suggested_actions = vec!["replace weak evidence reference".to_string()];
    let proof_commands = vec!["cargo-allow explain allow-weak".to_string()];
    let items = vec![WorklistItem {
        id: "work-weak-evidence-reference-0001",
        kind: "weak_evidence_reference",
        exception_kind: Some("unsafe"),
        family: Some("unsafe_block"),
        owner: Some("runtime"),
        classification: Some("reviewed_unsafe_boundary"),
        reason: Some("fixture"),
        created: None,
        review_after: None,
        expires: None,
        evidence_count: Some(1),
        selector_precision: Some(17),
        risk: "high",
        difficulty: "small",
        status: "evidence_missing",
        allow_id: Some("allow-weak"),
        candidate_ids: &[],
        finding_index: None,
        path: None,
        line: None,
        column: None,
        evidence_reference: Some(EvidenceReference {
            raw: "spreadsheet:manual-review",
            prefix: Some("spreadsheet"),
            target: Some("manual-review"),
            status: "unstructured",
            category: "unknown_prefix",
            message: "unrecognized evidence prefix; not locally validated",
        }),
        source_package: None,
        message: "allow-weak evidence `spreadsheet:manual-review`: unrecognized evidence prefix; not locally validated",
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }];

    let json = render_worklist_json(
        &items,
        WorklistFilters::default(),
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(47)),
    );

    assert!(json.contains("\"path\": null"));
    assert!(json.contains("\"evidence_reference\": {"));
    assert!(json.contains("\"raw\": \"spreadsheet:manual-review\""));
    assert!(json.contains("\"prefix\": \"spreadsheet\""));
    assert!(json.contains("\"target\": \"manual-review\""));
    assert!(json.contains("\"status\": \"unstructured\""));
    assert!(json.contains("\"category\": \"unknown_prefix\""));
    assert!(json.contains("\"selector_precision\": 17"));
    assert!(json.contains("\"source_package\": null"));

    let text = render_worklist_human(
        &items,
        WorklistFilters::default(),
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(47)),
    );

    assert!(
        text.contains("work-weak-evidence-reference-0001 (high, small) weak_evidence_reference")
    );
    assert!(text.contains(
        "  evidence reference: weak: spreadsheet:manual-review (status=unstructured, prefix=spreadsheet, target=manual-review)"
    ));
    assert!(
        text.contains("  evidence message: unrecognized evidence prefix; not locally validated")
    );
    assert!(text.contains("  selector_precision: 17"));
}

#[test]
fn worklist_human_renderer_shows_ledger_inspection_proof_commands() {
    let suggested_actions = vec!["review broad scope".to_string()];
    let proof_commands = vec![
        "cargo-allow explain allow-broad".to_string(),
        "cargo-allow list --allow-id allow-broad --format json".to_string(),
        "cargo-allow worklist --allow-id allow-broad --format json".to_string(),
        "cargo-allow worklist --broad-scope --format json".to_string(),
        "cargo-allow check --kind non-rust --mode no-new".to_string(),
        "cargo-allow worklist --item-kind broad_scope --format json".to_string(),
        "cargo-allow list --broad-scope --format json".to_string(),
        "cargo-allow worklist --kind non-rust --format json".to_string(),
        "cargo-allow worklist --format json".to_string(),
    ];
    let items = vec![WorklistItem {
        id: "work-broad-scope-0001",
        kind: "broad_scope",
        exception_kind: Some("non_rust_file"),
        family: Some("documentation"),
        owner: Some("docs"),
        classification: Some("reviewed_exception"),
        reason: Some("fixture"),
        created: None,
        review_after: None,
        expires: None,
        evidence_count: Some(1),
        selector_precision: Some(12),
        risk: "medium",
        difficulty: "small",
        status: "matched",
        allow_id: Some("allow-broad"),
        candidate_ids: &[],
        finding_index: None,
        path: Some("docs/**"),
        line: None,
        column: None,
        evidence_reference: None,
        source_package: None,
        message: "broad scope",
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }];

    let text = render_worklist_human(
        &items,
        WorklistFilters::default(),
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(47)),
    );

    assert!(text.contains("  proof: cargo-allow list --broad-scope --format json"));
    assert!(text.contains("  proof: cargo-allow worklist --kind non-rust --format json"));
    assert!(
        !text.contains("  proof: cargo-allow worklist --format json"),
        "human output should still avoid dumping every long-tail proof command"
    );
}

#[test]
fn worklist_status_style_is_fixed_label_only() {
    let suggested_actions = vec!["review stale allow".to_string()];
    let proof_commands = vec!["cargo-allow explain allow-stale".to_string()];
    let items = vec![WorklistItem {
        id: "work-stale-0001",
        kind: "stale_allow",
        exception_kind: Some("panic"),
        family: Some("unwrap"),
        owner: Some("parser"),
        classification: Some("reviewed_exception"),
        reason: Some("review this entry"),
        created: None,
        review_after: None,
        expires: None,
        evidence_count: None,
        selector_precision: Some(80),
        risk: "high",
        difficulty: "small",
        status: "stale",
        allow_id: Some("allow-stale"),
        candidate_ids: &[],
        finding_index: None,
        path: Some("src/lib.rs"),
        line: Some(4),
        column: Some(5),
        evidence_reference: None,
        source_package: Some("parser"),
        message: "repository message",
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }];
    let inventory =
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(47));

    let styled =
        render_worklist_human_styled(&items, WorklistFilters::default(), inventory, Style::ANSI);
    assert!(styled.contains("  status: \u{1b}[33mstale\u{1b}[0m"));
    assert!(styled.contains("\u{1b}[31mhigh\u{1b}[0m"));
    assert!(!styled.contains("message: \u{1b}"));

    let plain =
        render_worklist_human_styled(&items, WorklistFilters::default(), inventory, Style::PLAIN);
    assert!(!plain.contains('\u{1b}'));
}
