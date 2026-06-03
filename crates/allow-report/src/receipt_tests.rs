use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

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

#[test]
fn receipt_routes_evidence_repair_queues() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);
    context.policy_missing_evidence_entries = Some(4);
    context.weak_evidence_references = Some(3);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(json.contains("\"evidence_repair_queues\""));
    assert!(json.contains("\"signal\": \"broken_evidence_links\""));
    assert!(json.contains("\"label\": \"broken evidence links\""));
    assert!(json.contains("\"route_kind\": \"worklist_filter\""));
    assert!(json.contains("\"item_kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"worklist_filter\": \"broken_evidence\""));
    assert!(json.contains("\"count\": 2"));
    assert!(json.contains("\"command\": \"cargo-allow worklist --broken-evidence --format json\""));
    assert!(json.contains("\"signal\": \"missing_evidence\""));
    assert!(json.contains("\"label\": \"missing evidence\""));
    assert!(json.contains("\"route_kind\": \"worklist_filter\""));
    assert!(json.contains("\"item_kind\": \"missing_evidence\""));
    assert!(json.contains("\"worklist_filter\": \"missing_evidence\""));
    assert!(json.contains("\"count\": 4"));
    assert!(
        json.contains("\"command\": \"cargo-allow worklist --missing-evidence --format json\"")
    );
    assert!(json.contains("\"signal\": \"weak_evidence_references\""));
    assert!(json.contains("\"label\": \"weak evidence references\""));
    assert!(json.contains("\"item_kind\": \"weak_evidence_reference\""));
    assert!(json.contains("\"worklist_filter\": \"weak_evidence\""));
    assert!(json.contains("\"count\": 3"));
    assert!(json.contains("\"command\": \"cargo-allow worklist --weak-evidence --format json\""));
}

#[test]
fn receipt_omits_evidence_repair_queues_when_clean() {
    let json = render_receipt_with_context(
        "check",
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );

    assert!(!json.contains("\"evidence_repair_queues\""));
}

#[test]
fn receipt_can_include_source_exception_inventory() {
    let findings = vec![
        file_finding(FindingKind::Panic, "unwrap", "src/lib.rs"),
        file_finding(FindingKind::Unsafe, "unsafe_block", "src/ffi.rs"),
    ];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        outcome(MatchStatus::New, Some(1)),
    ];

    let json = render_receipt_with_context_and_inventory(
        "check",
        &findings,
        &outcomes,
        true,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );

    assert!(json.contains("\"source_inventory\""));
    assert!(json.contains("\"findings\": 2"));
    assert!(json.contains(
        "{\"kind\": \"panic\", \"total\": 1, \"matched\": 1, \"new\": 0, \"review_items\": 0}"
    ));
    assert!(json.contains(
        "{\"kind\": \"unsafe\", \"total\": 1, \"matched\": 0, \"new\": 1, \"review_items\": 1}"
    ));
    assert!(json.contains(
        "{\"kind\": \"panic\", \"family\": \"unwrap\", \"label\": \"panic.unwrap\", \"total\": 1, \"matched\": 1, \"new\": 0, \"review_items\": 0}"
    ));
    assert!(json.contains(
        "{\"kind\": \"unsafe\", \"family\": \"unsafe_block\", \"label\": \"unsafe.unsafe_block\", \"total\": 1, \"matched\": 0, \"new\": 1, \"review_items\": 1}"
    ));
}

#[test]
fn receipt_can_include_evidence_repair_queues_and_source_inventory() {
    let findings = vec![file_finding(
        FindingKind::Unsafe,
        "unsafe_block",
        "src/ffi.rs",
    )];
    let outcomes = vec![outcome(MatchStatus::EvidenceMissing, Some(0))];

    let json = render_receipt_with_context_and_inventory(
        "check",
        &findings,
        &outcomes,
        true,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );

    assert!(json.contains("\"evidence_repair_queues\""));
    assert!(json.contains("\"signal\": \"missing_evidence\""));
    assert!(json.contains("\"label\": \"missing evidence\""));
    assert!(json.contains("\"item_kind\": \"missing_evidence\""));
    assert!(json.contains("\"source_inventory\""));
}

fn outcome(status: MatchStatus, finding_index: Option<usize>) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: None,
        finding_index,
        message: String::new(),
        score: 0,
    }
}

fn file_finding(kind: FindingKind, family: &str, path: &str) -> Finding {
    Finding {
        kind,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("rust", "method_call"),
        message: "test finding".to_string(),
    }
}
