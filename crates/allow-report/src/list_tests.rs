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
        broken_evidence_references: 1,
        weak_evidence_references: 1,
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
            broken_evidence: true,
            weak_evidence: true,
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
    assert!(json.contains("\"location_drift\": false"));
    assert!(json.contains("\"broken_evidence\": true"));
    assert!(json.contains("\"weak_evidence\": true"));
    assert!(json.contains("\"allow_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"broken_evidence_references\": 1"));
    assert!(json.contains("\"weak_evidence_references\": 1"));
    assert!(json.contains("\"selector_precision\": 42"));
    assert!(json.contains("\"broad_scope\": true"));
    assert!(json.contains("\"review_after\": \"2026-07-01\""));
    assert!(!json.contains("\"expires\": null"));
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
    "location_drift": false,
    "baseline_debt": true,
    "broad_scope": false,
    "missing_evidence": false,
    "broken_evidence": true,
    "weak_evidence": true
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
      "broken_evidence_references": 1,
      "weak_evidence_references": 1,
      "selector_precision": 42,
      "broad_scope": true,
      "review_after": "2026-07-01",
      "reason": "generated baseline"
    }}
  ]
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);

    let text = render_list_human(
        &rows,
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(46)),
    );

    assert!(
        text.contains("inventory: source_tree/source_syntax via git_tracked; files scanned: 46")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("id\tstatus\tmatches\tkind\tfamily"));
    assert!(text.contains(
            "allow-json\tbaseline_debt\t1\tpanic\tunwrap\tparser\tbaseline_debt\tcrates/parser/src/lib.rs\tparser\t2\t1\t1\t42\ttrue\t2026-07-01\t-\tgenerated baseline"
        ));
    assert!(text.contains(CLAIM_BOUNDARY_TEXT));
}

#[test]
fn list_json_omits_unavailable_row_metadata() -> Result<(), String> {
    let rows = [ListRow {
        id: "allow-minimal",
        status: "matched",
        matches: 1,
        kind: "panic",
        family: None,
        owner: "parser",
        classification: "reviewed_exception",
        scope: "src/lib.rs",
        source_package: None,
        evidence_count: 0,
        broken_evidence_references: 0,
        weak_evidence_references: 0,
        selector_precision: 7,
        broad_scope: false,
        review_after: None,
        expires: None,
        reason: "validated invariant",
    }];

    let json = render_list_json(
        &rows,
        ListFilters::default(),
        InventoryContext::source_syntax("git_tracked", None, None),
    );
    let artifact: serde_json::Value = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    let row = artifact
        .get("allow_entries")
        .and_then(serde_json::Value::as_array)
        .and_then(|entries| entries.first())
        .ok_or_else(|| "list JSON should contain one allow entry".to_string())?;

    for field in ["family", "source_package", "review_after", "expires"] {
        assert!(
            row.get(field).is_none(),
            "unavailable {field} should be omitted"
        );
    }
    assert_eq!(
        row.get("id").and_then(serde_json::Value::as_str),
        Some("allow-minimal")
    );
    assert_eq!(
        row.get("reason").and_then(serde_json::Value::as_str),
        Some("validated invariant")
    );
    Ok(())
}

#[test]
fn render_list_human_columns_selects_subset_and_preserves_order() {
    // #2595: --columns id,status,reason produces a 3-column TSV with the
    // requested order, and the surrounding inventory/next-steps/claim
    // boundary text is unchanged.
    let rows = vec![ListRow {
        id: "allow-0001",
        status: "matched",
        matches: 2,
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "reviewed_exception",
        scope: "src/lib.rs",
        source_package: Some("allow-core"),
        evidence_count: 1,
        broken_evidence_references: 0,
        weak_evidence_references: 0,
        selector_precision: 7,
        broad_scope: false,
        review_after: Some("2026-10-01"),
        expires: None,
        reason: "validated invariant",
    }];

    let text = render_list_human_columns(
        &rows,
        InventoryContext::source_syntax("git_tracked", None, None),
        &[ListColumn::Id, ListColumn::Status, ListColumn::Reason],
    );

    // Header row contains exactly the three requested columns in order.
    assert!(
        text.contains("id\tstatus\treason\n"),
        "header should be id\\tstatus\\treason: {text}"
    );
    // The data row renders the corresponding cells in the same order.
    assert!(
        text.contains("allow-0001\tmatched\tvalidated invariant\n"),
        "data row should be allow-0001\\tmatched\\tvalidated invariant: {text}"
    );
    // Columns not selected must not appear as headers.
    assert!(
        !text.contains("matches\t"),
        "non-selected `matches` column should not appear: {text}"
    );
    assert!(
        !text.contains("owner\t"),
        "non-selected `owner` column should not appear: {text}"
    );
    // Shared scaffolding is preserved.
    assert!(text.contains("inventory: source_tree/source_syntax via git_tracked"));
    assert!(text.contains(CLAIM_BOUNDARY_TEXT));
}

#[test]
fn list_concise_summary_and_empty_states_are_explicit() {
    let long_reason = "this reason is intentionally long so the concise list view must bound repository-controlled free text without changing the wide projection";
    let rows = [ListRow {
        id: "allow-0001",
        status: "review_due",
        matches: 2,
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "reviewed_exception",
        scope: "src/lib.rs",
        source_package: Some("allow-core"),
        evidence_count: 1,
        broken_evidence_references: 1,
        weak_evidence_references: 2,
        selector_precision: 7,
        broad_scope: false,
        review_after: Some("2026-10-01"),
        expires: None,
        reason: long_reason,
    }];
    let filters = ListFilters {
        owner: Some("parser"),
        ..ListFilters::default()
    };
    let inventory = InventoryContext::source_syntax("git_tracked", None, Some(4));

    let text = render_list_human_concise(&rows, inventory, filters, ListColumn::DEFAULT);
    assert!(text.contains("summary: 1 allow entries shown"));
    assert!(text.contains("review_due: 1"));
    assert!(text.contains("broken evidence: 1"));
    assert!(text.contains("weak evidence: 2"));
    assert!(text.contains("entries:\n- [review_due] allow-0001\n"));
    assert!(text.contains("  kind: panic.unwrap\n"));
    assert!(text.contains("  matches: 2; evidence: 1 (broken: 1; weak: 2)\n"));
    assert!(text.contains("  reason: this reason is intentionally long"));
    assert!(!text.contains("id\tstatus\tkind\tscope\towner\treason"));
    assert!(text.contains('…'));
    assert!(!text.contains(long_reason));

    let filtered_empty = render_list_human_concise(&[], inventory, filters, ListColumn::DEFAULT);
    assert!(filtered_empty.contains("(no allow entries matched filters)"));

    let ledger_empty =
        render_list_human_concise(&[], inventory, ListFilters::default(), ListColumn::DEFAULT);
    assert!(ledger_empty.contains("(no allow entries are configured)"));

    let inventory_empty = render_list_human_concise(
        &[],
        inventory.with_empty_git_tracked(true),
        ListFilters::default(),
        ListColumn::DEFAULT,
    );
    assert!(inventory_empty.contains("(no tracked source files were found; inventory is empty)"));
}

#[test]
fn render_list_human_sanitizes_repository_control_characters() {
    let rows = vec![ListRow {
        id: "allow-\n001",
        status: "matched",
        matches: 1,
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser\tteam",
        classification: "approved",
        scope: "src/\u{1b}[31mevil.rs",
        source_package: None,
        evidence_count: 0,
        broken_evidence_references: 0,
        weak_evidence_references: 0,
        selector_precision: 1,
        broad_scope: false,
        review_after: None,
        expires: None,
        reason: "line one\r\nline two\u{7f}",
    }];

    let text = render_list_human_columns(
        &rows,
        InventoryContext::source_syntax("git_tracked", None, None),
        &[
            ListColumn::Id,
            ListColumn::Owner,
            ListColumn::Scope,
            ListColumn::Reason,
        ],
    );

    assert!(text.contains("allow-\\n001\tparser\\tteam\tsrc/�[31mevil.rs"));
    assert!(text.contains("line one\\r\\nline two�\n"));
    assert!(!text.contains("allow-\n001"));
    assert!(!text.contains("parser\tteam"));
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{7f}'));
}

#[test]
fn list_terminal_safety_is_preserved_in_concise_view() {
    let rows = [ListRow {
        id: "allow-\n001",
        status: "matched",
        matches: 1,
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser\tteam",
        classification: "approved",
        scope: "src/\u{1b}[31mevil.rs",
        source_package: None,
        evidence_count: 0,
        broken_evidence_references: 0,
        weak_evidence_references: 0,
        selector_precision: 1,
        broad_scope: false,
        review_after: None,
        expires: None,
        reason: "line one\r\nline two\u{7f}",
    }];
    let text = render_list_human_concise(
        &rows,
        InventoryContext::unknown_source_syntax(),
        ListFilters::default(),
        ListColumn::DEFAULT,
    );

    assert!(!text.contains("allow-\n001"));
    assert!(!text.contains("parser\tteam"));
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{7f}'));
    assert!(text.contains("allow-\\n001"));
    assert!(text.contains("parser\\tteam"));
}

#[test]
fn render_list_human_default_matches_full_header_row() {
    // #2595: when no column selection is made, render_list_human must
    // still produce the full 17-column header (backward compatibility).
    let rows: Vec<ListRow<'_>> = Vec::new();
    let text = render_list_human(
        &rows,
        InventoryContext::source_syntax("git_tracked", None, None),
    );
    assert!(
        text.contains(
            "id\tstatus\tmatches\tkind\tfamily\towner\tclassification\tscope\tsource_package\tevidence_count\tbroken_evidence_references\tweak_evidence_references\tselector_precision\tbroad_scope\treview_after\texpires\treason\n"
        ),
        "default render must keep the full 17-column header: {text}"
    );
}

#[test]
fn list_column_parse_csv_rejects_unknown_lists_valid_and_dedupes() {
    // Unknown name surfaces an error listing valid columns.
    let err = ListColumn::parse_csv("id,bogus,reason").expect_err("unknown column should error");
    assert!(
        err.contains("unknown --columns name `bogus`"),
        "error should name the bad column: {err}"
    );
    assert!(
        err.contains("valid columns:"),
        "error should list valid columns: {err}"
    );
    assert!(
        err.contains("source_package"),
        "valid-columns hint should include source_package: {err}"
    );

    // Valid selection parses in the requested order, with whitespace trimmed.
    let parsed = ListColumn::parse_csv(" id , status , reason ")
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse_csv should succeed: {err}")));
    assert_eq!(
        parsed,
        vec![ListColumn::Id, ListColumn::Status, ListColumn::Reason]
    );

    // Duplicate selection is rejected.
    let dup_err = ListColumn::parse_csv("id,id").expect_err("duplicate selection should error");
    assert!(
        dup_err.contains("duplicate --columns name `id`"),
        "error should name the duplicate: {dup_err}"
    );

    // Empty selection is rejected.
    let empty_err = ListColumn::parse_csv("").expect_err("empty selection should error");
    assert!(
        empty_err.contains("empty column name"),
        "error should explain the empty name: {empty_err}"
    );
}

#[test]
fn list_column_parse_csv_matches_case_insensitively() {
    // #2595 follow-up: column names are case-insensitive so operators don't
    // have to remember the exact casing. ID, Id, and id are equivalent.
    // The error message still lists canonical lowercase names.
    for mixed_case in ["ID", "Id", "iD", "STATUS", "Reason"] {
        let parsed = ListColumn::parse_csv(mixed_case).unwrap_or_else(|err| {
            std::panic::panic_any(format!("mixed-case {mixed_case} should parse: {err}"))
        });
        assert_eq!(
            parsed.len(),
            1,
            "mixed-case {mixed_case} should resolve to one column"
        );
    }
    // Mixed-case input resolves to the same variant as lowercase.
    let upper = ListColumn::parse_csv("ID,STATUS,REASON")
        .unwrap_or_else(|err| std::panic::panic_any(format!("uppercase should parse: {err}")));
    let lower = ListColumn::parse_csv("id,status,reason")
        .unwrap_or_else(|err| std::panic::panic_any(format!("lowercase should parse: {err}")));
    assert_eq!(
        upper, lower,
        "case variants should resolve to the same columns"
    );

    // Duplicate detection is case-insensitive too: ID and id are the same
    // column, so specifying both is a duplicate.
    let dup_err = ListColumn::parse_csv("ID,id").expect_err("case-variant duplicate should error");
    assert!(
        dup_err.contains("duplicate --columns name"),
        "case-variant duplicate should be detected: {dup_err}"
    );
}

#[test]
fn list_column_all_is_canonical_seventeen_in_order() {
    // The default projection is exactly the 17 columns in the pre-#2595
    // header order. If this changes, the backward-compat assertion above
    // and the help text in list_args.rs both need updating.
    assert_eq!(ListColumn::ALL.len(), 17);
    let headers: Vec<&str> = ListColumn::ALL.iter().map(|c| c.header()).collect();
    assert_eq!(
        headers,
        vec![
            "id",
            "status",
            "matches",
            "kind",
            "family",
            "owner",
            "classification",
            "scope",
            "source_package",
            "evidence_count",
            "broken_evidence_references",
            "weak_evidence_references",
            "selector_precision",
            "broad_scope",
            "review_after",
            "expires",
            "reason",
        ]
    );
}
