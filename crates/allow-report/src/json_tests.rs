use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, MatchOutcome, MatchStatus, Selector,
    Span, StructuralIdentity,
};
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn json_contains_claim_boundary() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax(
            "filesystem_fallback",
            Some("fixtures/source-snapshot"),
            Some(7),
            None,
        ),
    );
    assert!(CLAIM_BOUNDARY.contains(&"source_tree_inventory"));
    assert!(SCANNER_LIMITATIONS.contains(&"cargo_metadata_not_invoked"));
    assert_eq!(CLAIM_BOUNDARY.len(), SCANNER_LIMITATIONS.len() + 3);
    assert!(json.contains("source_tree_inventory"));
    assert!(json.contains("cargo_metadata_not_invoked"));
    assert!(json.contains("cargo_commands_not_invoked"));
    assert!(json.contains("rustc_not_invoked"));
    assert!(json.contains("clippy_not_invoked"));
    assert!(json.contains("build_scripts_not_executed"));
    assert!(json.contains("proc_macros_not_executed"));
    assert!(json.contains("macro_expansion_not_analyzed"));
    assert!(json.contains("macro_token_tree_contents_not_analyzed"));
    assert!(json.contains("repository_code_not_executed"));
}

#[test]
fn json_report_exposes_v1_schema_contract() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax(
            "filesystem_fallback",
            Some("fixtures/source-snapshot"),
            Some(7),
            None,
        ),
    );
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"schema_id\": \"cargo-allow.report.v1\""));
    assert!(json.contains("\"failed\": false"));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"scope\": \"source_tree\""));
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"filesystem_fallback\""));
    assert!(json.contains("\"root\": \"fixtures/source-snapshot\""));
    assert!(json.contains("\"files_scanned\": 7"));
    assert!(json.contains("\"review_due\": 0"));
    assert!(json.contains("\"baseline_debt\": 0"));
    assert!(json.contains("\"trend\""));
    assert!(json.contains("\"review_items\": 0"));
}

#[test]
fn json_report_exposes_inventory_completeness() {
    let context = ReportContext::source_syntax(
        "filesystem_fallback",
        Some("fixtures/source-snapshot"),
        Some(7),
        None,
    )
    .with_inventory_completeness("fallback");
    let json = render_json_with_context("audit", &[], &[], false, context);

    assert!(json.contains("\"completeness\": \"fallback\""));
}

#[test]
fn json_report_matches_empty_audit_golden_contract() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax(
            "filesystem_fallback",
            Some("fixtures/source-snapshot"),
            Some(7),
            None,
        ),
    );
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.report.v1",
  "tool": "cargo-allow",
  "command": "audit",
  "status": "passed",
  "failed": false,
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "filesystem_fallback",
    "root": "fixtures/source-snapshot",
    "files_scanned": 7
  }},
  "summary": {{
    "findings": 0,
    "outcomes": 0,
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
  "trend": {{
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
  "evidence_repair_queues": [

  ],
  "outcomes": [

  ],
  "findings": [

  ]
}}"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );

    assert_eq!(json, expected);
}

#[test]
#[should_panic(expected = "report artifacts support only audit, check, or diff commands")]
fn json_report_rejects_unknown_artifact_command() {
    let _ = render_json_with_context("explain", &[], &[], false, ReportContext::default());
}

#[test]
#[should_panic(expected = "fixed artifact preamble requires a fixed-command artifact contract")]
fn fixed_artifact_preamble_rejects_variable_command_contract() {
    let mut out = String::new();
    crate::json::push_json_fixed_artifact_preamble(
        &mut out,
        crate::contracts::REPORT_ARTIFACT,
        InventoryContext::default(),
    );
}

#[test]
fn json_report_exposes_trend_metrics() {
    let outcomes = vec![
        outcome(MatchStatus::New, Some(0)),
        outcome(MatchStatus::EvidenceMissing, Some(1)),
        outcome(MatchStatus::Stale, None),
    ];

    let json = render_json("audit", &[], &outcomes, false);

    assert!(json.contains("\"trend\""));
    assert!(json.contains("\"review_items\": 3"));
    assert!(json.contains("\"new\": 1"));
    assert!(json.contains("\"stale\": 1"));
    assert!(json.contains("\"evidence_missing\": 1"));
    assert!(json.contains("\"baseline_debt\": 0"));
}

#[test]
fn json_report_trend_counts_policy_baseline_debt_context() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, Some(3)),
    );

    assert!(json.contains("\"review_items\": 3"));
    assert!(json.contains("\"baseline_debt\": 3"));
    assert!(json.contains("\"policy_baseline_debt\": 3"));
}

#[test]
fn json_report_trend_counts_broken_evidence_links_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);
    let json = render_json_with_context("audit", &[], &[], false, context);

    assert!(json.contains("\"review_items\": 2"));
    assert!(json.contains("\"broken_evidence_links\": 2"));
}

#[test]
fn json_report_trend_counts_weak_evidence_references_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.weak_evidence_references = Some(2);
    let json = render_json_with_context("audit", &[], &[], false, context);

    assert!(json.contains("\"review_items\": 2"));
    assert!(json.contains("\"weak_evidence_references\": 2"));
}

#[test]
fn json_report_routes_evidence_repair_queues() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);
    context.policy_missing_evidence_entries = Some(4);
    context.weak_evidence_references = Some(3);
    let json = render_json_with_context("audit", &[], &[], false, context);

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
fn json_audit_report_routes_remediation_roadmap() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(5));
    context.policy_missing_evidence_entries = Some(4);
    context.broken_evidence_links = Some(2);
    context.weak_evidence_references = Some(3);
    let outcomes = vec![
        outcome(MatchStatus::New, Some(0)),
        outcome(MatchStatus::Expired, None),
        outcome(MatchStatus::ReviewDue, None),
        outcome(MatchStatus::Stale, None),
        outcome(MatchStatus::Ambiguous, None),
        outcome(MatchStatus::InvalidSelector, None),
        outcome(MatchStatus::MissingRequiredField, None),
        outcome(MatchStatus::EvidenceMissing, None),
    ];

    let json = render_json_with_context("audit", &[], &outcomes, true, context);

    assert!(json.contains("\"audit_remediation_roadmap\""));
    assert!(json.contains("\"signal\": \"new_unreceipted\""));
    assert!(json.contains("\"label\": \"new unreceipted\""));
    assert!(json.contains("\"route_kind\": \"worklist_status\""));
    assert!(json.contains("\"item_kind\": \"new_unreceipted_finding\""));
    assert!(json.contains("\"worklist_status\": \"new\""));
    assert!(json.contains("\"command\": \"cargo-allow worklist --status new --format json\""));
    assert!(json.contains("\"signal\": \"expired\""));
    assert!(json.contains("\"command\": \"cargo-allow worklist --status expired --format json\""));
    assert!(json.contains("\"signal\": \"review_due\""));
    assert!(
        json.contains("\"command\": \"cargo-allow worklist --status review_due --format json\"")
    );
    assert!(json.contains("\"signal\": \"stale\""));
    assert!(json.contains("\"route_kind\": \"prune_stale\""));
    assert!(json.contains("\"item_kind\": \"stale_allow\""));
    assert!(json.contains("\"command\": \"cargo-allow prune --stale --dry-run --format json --output target/cargo-allow/prune.json\""));
    assert!(json.contains("\"signal\": \"ambiguous\""));
    assert!(
        json.contains("\"command\": \"cargo-allow worklist --status ambiguous --format json\"")
    );
    assert!(json.contains("\"signal\": \"invalid_selector\""));
    assert!(
        json.contains(
            "\"command\": \"cargo-allow worklist --status invalid_selector --format json\""
        )
    );
    assert!(json.contains("\"signal\": \"missing_required_field\""));
    assert!(json.contains(
        "\"command\": \"cargo-allow worklist --status missing_required_field --format json\""
    ));
    assert!(json.contains("\"signal\": \"missing_evidence\""));
    assert!(json.contains("\"route_kind\": \"worklist_filter\""));
    assert!(json.contains("\"item_kind\": \"missing_evidence\""));
    assert!(json.contains("\"worklist_filter\": \"missing_evidence\""));
    assert!(
        json.contains("\"command\": \"cargo-allow worklist --missing-evidence --format json\"")
    );
    assert!(json.contains("\"signal\": \"broken_evidence_links\""));
    assert!(json.contains("\"route_kind\": \"worklist_filter\""));
    assert!(json.contains("\"item_kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"worklist_filter\": \"broken_evidence\""));
    assert!(json.contains("\"command\": \"cargo-allow worklist --broken-evidence --format json\""));
    assert!(json.contains("\"signal\": \"weak_evidence_references\""));
    assert!(json.contains("\"worklist_filter\": \"weak_evidence\""));
    assert!(json.contains("\"command\": \"cargo-allow worklist --weak-evidence --format json\""));
    assert!(json.contains("\"signal\": \"baseline_debt\""));
    assert!(json.contains("\"item_kind\": \"baseline_debt\""));
    assert!(json.contains("\"worklist_filter\": \"baseline_debt\""));
    assert!(json.contains("\"command\": \"cargo-allow worklist --baseline-debt --format json\""));
    assert!(json.contains("\"count\": 5"));
}

#[test]
fn json_audit_report_omits_remediation_roadmap_when_clean() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );

    assert!(!json.contains("\"audit_remediation_roadmap\""));
}

#[test]
fn json_check_report_omits_audit_remediation_roadmap() {
    let json = render_json("check", &[], &[outcome(MatchStatus::New, Some(0))], true);

    assert!(!json.contains("\"audit_remediation_roadmap\""));
}

#[test]
fn json_report_always_includes_evidence_repair_queues_even_when_clean() {
    // #1858: evidence_repair_queues is always present (empty array when clean)
    // for consistent empty-handling across artifacts.
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, None),
    );

    assert!(
        json.contains("\"evidence_repair_queues\":"),
        "json report should always include evidence_repair_queues (empty when clean): {json}"
    );
}

#[test]
fn json_report_trend_counts_policy_missing_evidence_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.policy_missing_evidence_entries = Some(4);
    let json = render_json_with_context("audit", &[], &[], false, context);

    assert!(json.contains("\"review_items\": 4"));
    assert!(json.contains("\"policy_missing_evidence\": 4"));
}

#[test]
fn matched_policy_missing_evidence_counts_only_matched_non_baseline_entries() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry("allow-matched", "reviewed", &[]));
    cfg.allow
        .push(test_entry("allow-evidenced", "reviewed", &["test:covered"]));
    cfg.allow.push(test_entry("allow-stale", "reviewed", &[]));
    cfg.allow
        .push(test_entry("allow-baseline", "baseline_debt", &[]));
    let outcomes = vec![
        outcome_with_allow(MatchStatus::Matched, Some("allow-matched")),
        outcome_with_allow(MatchStatus::Matched, Some("allow-evidenced")),
        outcome_with_allow(MatchStatus::Stale, Some("allow-stale")),
        outcome_with_allow(MatchStatus::Matched, Some("allow-baseline")),
    ];

    assert_eq!(matched_policy_missing_evidence_entries(&cfg, &outcomes), 1);
}

#[test]
fn json_report_exposes_source_package_context_on_findings() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    let findings = vec![Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates/parser/src/lib.rs"),
        span: Some(Span {
            line: 12,
            column: 8,
        }),
        identity,
        message: "unwrap call".to_string(),
        ledger: None,
    }];

    let json = render_json("audit", &findings, &[], false);

    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"path\": \"crates/parser/src/lib.rs\""));
}

#[test]
fn json_report_omits_unavailable_finding_metadata_and_keeps_navigation_nulls() -> Result<(), String>
{
    let finding = Finding {
        kind: FindingKind::Panic,
        family: None,
        path: PathBuf::from("src\\lib.rs"),
        span: None,
        identity: StructuralIdentity::new("rust", "macro_call"),
        message: "panic macro".to_string(),
        ledger: None,
    };
    let value: Value = serde_json::from_str(&render_json("audit", &[finding], &[], false))
        .map_err(|error| format!("sparse report should render valid JSON: {error}"))?;
    let row = value
        .pointer("/findings/0")
        .ok_or_else(|| "sparse report should include a finding row".to_string())?;
    for field in ["family", "source_package"] {
        if row.get(field).is_some() {
            return Err(format!("sparse report should omit {field}"));
        }
    }
    if row.get("line") != Some(&Value::Null) || row.get("container") != Some(&Value::Null) {
        return Err(
            "unavailable navigation and identity fields should remain nullable".to_string(),
        );
    }

    Ok(())
}

#[test]
fn json_report_exposes_source_exception_inventory() {
    let findings = vec![
        file_finding(FindingKind::Panic, "unwrap", "src/lib.rs"),
        file_finding(FindingKind::Unsafe, "unsafe_block", "src/ffi.rs"),
    ];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        outcome(MatchStatus::New, Some(1)),
    ];

    let json = render_json("audit", &findings, &outcomes, false);

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

fn outcome_with_allow(status: MatchStatus, allow_id: Option<&str>) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(ToOwned::to_owned),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: String::new(),
        score: 0,
    }
}

fn test_entry(id: &str, classification: &str, evidence: &[&str]) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: classification.to_string(),
        reason: "fixture".to_string(),
        evidence: evidence.iter().map(|item| (*item).to_string()).collect(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-08-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
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
