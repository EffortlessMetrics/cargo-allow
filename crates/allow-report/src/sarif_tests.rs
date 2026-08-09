use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

fn context(source: &'static str) -> ReportContext<'static> {
    ReportContext::source_syntax(source, None, None, None)
}

#[test]
fn sarif_report_projects_diff_analysis_context() {
    let context = ReportContext {
        diff_analysis: Some(DiffAnalysisContext {
            result_class: "head_partial",
            base_revision: Some("origin/main"),
            head_revision: Some("HEAD"),
            base_inventory_complete: true,
            base_scanner_complete: true,
            head_inventory_complete: true,
            head_scanner_complete: false,
            introduced: 1,
            retained: 2,
            removed: 0,
        }),
        ..context("git_tracked")
    };
    let sarif = render_sarif_with_context("diff", &[], &[], true, context);
    assert!(sarif.contains("\"diff_analysis\": {"));
    assert!(sarif.contains("\"result_class\": \"head_partial\""));
    assert!(sarif.contains("\"head_scanner_complete\": false"));
    assert!(sarif.contains("\"introduced\": 1"));
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
            candidate_ids: Vec::new(),
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
fn sarif_results_include_dedup_fingerprints_and_invocation_metadata() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.normalized_snippet_hash = Some("fnv1a64:snippet".to_string());
    let findings = vec![Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates/parser/src/lib.rs"),
        span: Some(Span { line: 4, column: 9 }),
        identity,
        message: "unwrap call".to_string(),
        ledger: None,
    }];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted unwrap".to_string(),
        score: 0,
    }];
    let mut context = context("git_tracked");
    context.started_at = Some("2026-07-28T23:00:00Z");

    let sarif = render_sarif_with_context("check", &findings, &outcomes, true, context);
    let value: serde_json::Value = serde_json::from_str(&sarif)
        .unwrap_or_else(|err| std::panic::panic_any(format!("SARIF should be valid JSON: {err}")));
    let run = |path: &str| {
        value.pointer(path).unwrap_or_else(|| {
            std::panic::panic_any(format!("SARIF test path should exist: {path}"))
        })
    };
    assert_eq!(
        run("/runs/0/automationDetails/id").as_str(),
        Some("cargo-allow/check")
    );
    assert_eq!(
        run("/runs/0/invocations/0/executionSuccessful").as_bool(),
        Some(false)
    );
    assert_eq!(
        run("/runs/0/invocations/0/startTimeUtc").as_str(),
        Some("2026-07-28T23:00:00Z")
    );
    assert!(
        run("/runs/0/invocations/0/endTimeUtc")
            .as_str()
            .is_some_and(|value| value.ends_with('Z'))
    );
    assert!(
        run("/runs/0/tool/driver/rules")
            .as_array()
            .is_some_and(|rules| {
                rules.iter().any(|rule| {
                    rule.get("id").and_then(serde_json::Value::as_str)
                        == Some("cargo-allow/location_drift")
                })
            })
    );

    assert!(
        run("/runs/0/results/0/partialFingerprints/primaryLocationLineHash")
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:v1:"))
    );
    assert!(
        run("/runs/0/results/0/partialFingerprints/stableResultHash")
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:v1:"))
    );
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
        ledger: None,
    }];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
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
    assert!(sarif.contains("\"evidence_repair_queues\""));
    assert!(sarif.contains("\"signal\": \"broken_evidence_links\""));
    assert!(sarif.contains("\"label\": \"broken evidence links\""));
    assert!(sarif.contains("\"route_kind\": \"worklist_filter\""));
    assert!(sarif.contains("\"item_kind\": \"broken_evidence_link\""));
    assert!(sarif.contains("\"worklist_filter\": \"broken_evidence\""));
    assert!(
        sarif.contains("\"command\": \"cargo-allow worklist --broken-evidence --format json\"")
    );
    assert!(sarif.contains("\"signal\": \"missing_evidence\""));
    assert!(sarif.contains("\"label\": \"missing evidence\""));
    assert!(sarif.contains("\"route_kind\": \"worklist_filter\""));
    assert!(sarif.contains("\"item_kind\": \"missing_evidence\""));
    assert!(sarif.contains("\"worklist_filter\": \"missing_evidence\""));
    assert!(
        sarif.contains("\"command\": \"cargo-allow worklist --missing-evidence --format json\"")
    );
    assert!(sarif.contains("\"signal\": \"weak_evidence_references\""));
    assert!(sarif.contains("\"label\": \"weak evidence references\""));
    assert!(sarif.contains("\"item_kind\": \"weak_evidence_reference\""));
    assert!(sarif.contains("\"worklist_filter\": \"weak_evidence\""));
    assert!(sarif.contains("\"command\": \"cargo-allow worklist --weak-evidence --format json\""));
    assert!(sarif.contains("\"results\": [\n\n      ]"));
}

#[test]
fn sarif_run_properties_route_outcome_evidence_missing_repair_queue() {
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::EvidenceMissing,
        allow_id: Some("allow-unsafe-0001".to_string()),
        candidate_ids: Vec::new(),
        finding_index: None,
        message: "unsafe allow entry requires evidence".to_string(),
        score: 0,
    }];

    let sarif = render_sarif_with_context("check", &[], &outcomes, true, context("git_tracked"));

    assert!(sarif.contains("\"evidence_repair_queues\""));
    assert!(sarif.contains("\"signal\": \"missing_evidence\""));
    assert!(sarif.contains("\"label\": \"missing evidence\""));
    assert!(sarif.contains("\"route_kind\": \"worklist_filter\""));
    assert!(sarif.contains("\"item_kind\": \"missing_evidence\""));
    assert!(sarif.contains("\"worklist_filter\": \"missing_evidence\""));
    assert!(sarif.contains("\"count\": 1"));
    assert!(
        sarif.contains("\"command\": \"cargo-allow worklist --missing-evidence --format json\"")
    );
}

#[test]
fn sarif_run_properties_omit_evidence_repair_queues_when_clean() {
    let sarif = render_sarif_with_context("check", &[], &[], false, context("git_tracked"));

    assert!(!sarif.contains("\"evidence_repair_queues\""));
}

fn file_finding(kind: FindingKind, family: &str, path: &str) -> Finding {
    Finding {
        kind,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", "tracked_file"),
        message: "tracked non-Rust file".to_string(),
        ledger: None,
    }
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
