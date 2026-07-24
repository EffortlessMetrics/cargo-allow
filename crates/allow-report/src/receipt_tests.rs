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
    "location_drift": 0,
    "stale": 0,
    "ambiguous": 0,
    "invalid_selector": 0,
    "evidence_missing": 0,
    "missing_required_field": 0,
    "baseline_debt": 0
  }},
  "advisory": {{
    "review_items": 0,
    "new": 0,
    "expired": 0,
    "review_due": 0,
    "location_drift": 0,
    "stale": 0,
    "ambiguous": 0,
    "invalid_selector": 0,
    "missing_required_field": 0,
    "evidence_missing": 0,
    "baseline_debt": 0
  }},
  "evidence_repair_queues": []
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );

    let actual_value = serde_json::from_str::<serde_json::Value>(&json);
    let expected_value = serde_json::from_str::<serde_json::Value>(&expected);
    assert!(actual_value.is_ok(), "typed receipt must remain valid JSON");
    assert!(
        expected_value.is_ok(),
        "golden receipt must remain valid JSON"
    );
    assert_eq!(actual_value.ok(), expected_value.ok());
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
fn receipt_emits_provenance_binding_when_git_sha_and_policy_digest_present() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.git_sha = Some("abc123def456");
    context.policy_digest =
        Some("sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08");

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(
        json.contains("\"git_sha\": \"abc123def456\""),
        "receipt must emit git_sha when present"
    );
    assert!(
        json.contains("\"policy_digest\": \"sha256:"),
        "receipt must emit policy_digest when present"
    );
}

#[test]
fn receipt_omits_provenance_binding_when_absent() {
    let context = ReportContext::source_syntax("git_tracked", None, None, None);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(
        !json.contains("\"git_sha\""),
        "receipt must not emit git_sha when absent"
    );
    assert!(
        !json.contains("\"policy_digest\""),
        "receipt must not emit policy_digest when absent"
    );
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
fn receipt_advisory_counts_policy_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(3));
    context.policy_missing_evidence_entries = Some(4);
    context.broken_evidence_links = Some(2);
    context.weak_evidence_references = Some(1);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::ReviewDue,
        allow_id: Some("review".to_string()),
        candidate_ids: Vec::new(),
        finding_index: None,
        message: "review_due".to_string(),
        score: 0,
    }];
    let json = render_receipt_with_context("check", &outcomes, false, context);

    assert!(json.contains("\"advisory\": {"));
    assert!(json.contains("\"review_items\": 11"));
    assert!(json.contains("\"review_due\": 1"));
    assert!(json.contains("\"policy_missing_evidence\": 4"));
    assert!(json.contains("\"broken_evidence_links\": 2"));
    assert!(json.contains("\"weak_evidence_references\": 1"));
}

#[test]
fn receipt_always_includes_evidence_repair_queues_even_when_clean() {
    // #1858: evidence_repair_queues is always present (empty array when clean)
    // so downstream consumers can distinguish "feature off" from "zero count".
    let json = render_receipt_with_context(
        "check",
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );

    assert!(
        json.contains("\"evidence_repair_queues\": []"),
        "receipt should always include evidence_repair_queues (empty when clean): {json}"
    );
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

    let value = serde_json::from_str::<serde_json::Value>(&json);
    assert!(value.is_ok(), "typed receipt must remain valid JSON");
    let Some(value) = value.ok() else {
        return;
    };
    let source_inventory = value.get("source_inventory");
    assert!(
        source_inventory.is_some(),
        "receipt must include source_inventory"
    );
    let Some(source_inventory) = source_inventory else {
        return;
    };
    assert_eq!(
        source_inventory.get("findings").and_then(|v| v.as_u64()),
        Some(2)
    );
    let by_kind = source_inventory.get("by_kind").and_then(|v| v.as_array());
    assert!(by_kind.is_some(), "source inventory must include by_kind");
    let Some(by_kind) = by_kind else {
        return;
    };
    assert!(by_kind.iter().any(|row| {
        row.get("kind").and_then(|v| v.as_str()) == Some("panic")
            && row.get("matched").and_then(|v| v.as_u64()) == Some(1)
    }));
    assert!(by_kind.iter().any(|row| {
        row.get("kind").and_then(|v| v.as_str()) == Some("unsafe")
            && row.get("new").and_then(|v| v.as_u64()) == Some(1)
    }));
    let by_family = source_inventory.get("by_family").and_then(|v| v.as_array());
    assert!(
        by_family.is_some(),
        "source inventory must include by_family"
    );
    let Some(by_family) = by_family else {
        return;
    };
    assert!(by_family.iter().any(|row| {
        row.get("label").and_then(|v| v.as_str()) == Some("panic.unwrap")
            && row.get("matched").and_then(|v| v.as_u64()) == Some(1)
    }));
    assert!(by_family.iter().any(|row| {
        row.get("label").and_then(|v| v.as_str()) == Some("unsafe.unsafe_block")
            && row.get("new").and_then(|v| v.as_u64()) == Some(1)
    }));
}

#[test]
fn receipt_error_diagnostic_escapes_quotes_and_control_characters() {
    let diagnostic = "invalid policy: \"mode\"\\path\nline\t\0";

    let json = render_error_receipt(
        diagnostic,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );
    let value = serde_json::from_str::<serde_json::Value>(&json);

    assert!(value.is_ok(), "error receipt must remain valid JSON");
    let Some(value) = value.ok() else {
        return;
    };
    assert_eq!(value.get("status").and_then(|v| v.as_str()), Some("error"));
    assert_eq!(value.get("failed").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        value.get("diagnostic").and_then(|v| v.as_str()),
        Some(diagnostic)
    );
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

#[test]
fn receipt_records_blocking_divergence_distinct_from_advisory_review_items() {
    // #1945: a blocking drain_expired divergence fails the run but was invisible
    // in the receipt because only the advisory mirror count was recorded. The
    // blocking count must now surface in the counts block, and it must not be
    // laundered into the advisory review-item tally.
    let mut blocking = ReportContext::source_syntax("git_tracked", None, None, None);
    blocking.blocking_divergence_entries = Some(2);
    let blocking_json = render_receipt_with_context("check", &[], true, blocking);
    assert!(
        blocking_json.contains("\"blocking_divergence\": 2"),
        "receipt counts must record the blocking divergence: {blocking_json}"
    );
    assert!(
        blocking_json.contains("\"review_items\": 0"),
        "a blocking divergence must not inflate advisory review items: {blocking_json}"
    );

    // By contrast, an advisory mirror divergence does count as a review item and
    // is not reported as a blocking divergence.
    let mut advisory = ReportContext::source_syntax("git_tracked", None, None, None);
    advisory.mirror_divergence_entries = Some(2);
    let advisory_json = render_receipt_with_context("check", &[], false, advisory);
    assert!(
        advisory_json.contains("\"review_items\": 2"),
        "an advisory mirror divergence counts as a review item: {advisory_json}"
    );
    assert!(
        !advisory_json.contains("\"blocking_divergence\""),
        "an advisory-only divergence emits no blocking count: {advisory_json}"
    );
}

fn outcome(status: MatchStatus, finding_index: Option<usize>) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: None,
        candidate_ids: Vec::new(),
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
        ledger: None,
    }
}
