use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

fn context(source: &'static str) -> ReportContext<'static> {
    ReportContext::source_syntax(source, None, None, None)
}

#[test]
fn sarif_report_emits_non_matched_results_with_locations() {
    let findings = vec![file_finding(
        FindingKind::NonRustFile,
        "shell_script",
        "scripts/new.sh",
    )];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: Some(0),
            message: "unreceipted shell script at scripts/new.sh".to_string(),
            score: 0,
        },
    ];

    let sarif =
        render_sarif_with_context("check", &findings, &outcomes, true, context("git_tracked"));

    assert!(sarif.contains("\"version\": \"2.1.0\""));
    assert!(sarif.contains("\"name\": \"cargo-allow\""));
    assert!(sarif.contains("\"ruleId\": \"cargo-allow/new\""));
    assert!(sarif.contains("\"level\": \"error\""));
    assert!(sarif.contains("\"uri\": \"scripts/new.sh\""));
    assert!(sarif.contains("\"startLine\": 1"));
    assert!(sarif.contains("\"source_tree_inventory\""));
    assert!(sarif.contains("\"cargo_commands_not_invoked\""));
    assert!(!sarif.contains("\"ruleId\": \"cargo-allow/matched\""));
}

#[test]
fn sarif_result_properties_include_source_package_context() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    let findings = vec![Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates/parser/src/lib.rs"),
        span: Some(Span { line: 4, column: 9 }),
        identity,
        message: "unwrap call".to_string(),
    }];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        finding_index: Some(0),
        message: "unreceipted unwrap".to_string(),
        score: 0,
    }];

    let sarif = render_sarif("check", &findings, &outcomes, true);

    assert!(sarif.contains("\"source_package\": \"parser\""));
    assert!(sarif.contains("\"uri\": \"crates/parser/src/lib.rs\""));
}

#[test]
fn sarif_run_properties_include_policy_evidence_health_counts() {
    let mut context = context("git_tracked");
    context.baseline_debt_entries = Some(3);
    context.policy_missing_evidence_entries = Some(2);
    context.broken_evidence_links = Some(1);
    context.weak_evidence_references = Some(1);

    let sarif = render_sarif_with_context("audit", &[], &[], false, context);

    assert!(sarif.contains("\"policy_baseline_debt\": 3"));
    assert!(sarif.contains("\"policy_missing_evidence\": 2"));
    assert!(sarif.contains("\"broken_evidence_links\": 1"));
    assert!(sarif.contains("\"weak_evidence_references\": 1"));
    assert!(sarif.contains("\"results\": [\n\n      ]"));
}

fn file_finding(kind: FindingKind, family: &str, path: &str) -> Finding {
    Finding {
        kind,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", "tracked_file"),
        message: "tracked non-Rust file".to_string(),
    }
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
