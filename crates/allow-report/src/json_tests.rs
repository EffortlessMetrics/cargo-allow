use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
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
