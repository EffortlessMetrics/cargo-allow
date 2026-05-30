use super::*;

#[test]
fn receipt_exposes_v1_schema_contract() {
    let json = render_receipt_with_context(
        "check",
        &[],
        true,
        ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
            None,
        ),
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

#[test]
fn receipt_matches_empty_check_golden_contract() {
    let json = render_receipt_with_context(
        "check",
        &[],
        false,
        ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
            None,
        ),
    );
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.receipt.v1",
  "tool": "cargo-allow",
  "command": "check",
  "status": "passed",
  "failed": false,
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 42
  }},
  "counts": {{
    "matched": 0,
    "new": 0,
    "expired": 0,
    "review_due": 0,
    "stale": 0,
    "ambiguous": 0,
    "invalid_selector": 0,
    "evidence_missing": 0,
    "missing_required_field": 0,
    "baseline_debt": 0
  }}
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );

    assert_eq!(json, expected);
}

#[test]
#[should_panic(expected = "receipt artifacts support only the check command")]
fn receipt_rejects_unknown_artifact_command() {
    let _ = render_receipt_with_context("audit", &[], false, ReportContext::default());
}

#[test]
fn receipt_counts_policy_baseline_debt_context() {
    let json = render_receipt_with_context(
        "check",
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, Some(3)),
    );

    assert!(json.contains("\"baseline_debt\": 0"));
    assert!(json.contains("\"policy_baseline_debt\": 3"));
}

#[test]
fn receipt_counts_broken_evidence_links_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(json.contains("\"broken_evidence_links\": 2"));
}

#[test]
fn receipt_counts_weak_evidence_references_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.weak_evidence_references = Some(2);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(json.contains("\"weak_evidence_references\": 2"));
}

#[test]
fn receipt_counts_policy_missing_evidence_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.policy_missing_evidence_entries = Some(4);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(json.contains("\"policy_missing_evidence\": 4"));
}
