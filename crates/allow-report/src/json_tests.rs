use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, MatchOutcome, MatchStatus, Selector,
    Span, StructuralIdentity,
};
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
    assert_eq!(CLAIM_BOUNDARY.len(), SCANNER_LIMITATIONS.len() + 2);
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
#[should_panic(expected = "report artifacts support only audit, check, or diff commands")]
fn json_report_rejects_unknown_artifact_command() {
    let _ = render_json_with_context("explain", &[], &[], false, ReportContext::default());
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
    }];

    let json = render_json("audit", &findings, &[], false);

    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"path\": \"crates/parser/src/lib.rs\""));
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

fn outcome_with_allow(status: MatchStatus, allow_id: Option<&str>) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(ToOwned::to_owned),
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
