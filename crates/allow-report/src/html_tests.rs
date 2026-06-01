use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

fn context(source: &'static str) -> ReportContext<'static> {
    ReportContext::source_syntax(source, None, None, None)
}

#[test]
fn html_report_summarizes_audit_posture() {
    let findings = vec![file_finding(
        FindingKind::NonRustFile,
        "shell_script",
        "scripts/new.sh",
    )];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        finding_index: Some(0),
        message: "unreceipted shell script at scripts/new.sh".to_string(),
        score: 0,
    }];

    let html =
        render_html_with_context("audit", &findings, &outcomes, true, context("git_tracked"));

    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("<h1>cargo-allow audit</h1>"));
    assert!(html.contains("Result: failed"));
    assert!(html.contains("<h2>Audit Summary</h2>"));
    assert!(html.contains("<h2>Source Exception Inventory</h2>"));
    assert!(html.contains("Findings inventoried: <code>1</code>"));
    assert!(html.contains("<code class=\"kind\">non_rust_file</code>"));
    assert!(html.contains("<code class=\"family\">non_rust_file.shell_script</code>"));
    assert!(html.contains("<h2>Non-Rust File Inventory</h2>"));
    assert!(html.contains("<code>new</code>"));
    assert!(html.contains("<code>scripts/new.sh</code>"));
    assert!(html.contains("did not invoke Cargo metadata"));
    assert!(html.contains("external evidence tools"));
}

#[test]
fn html_audit_report_counts_policy_missing_evidence_context() {
    let mut context = context("git_tracked");
    context.policy_missing_evidence_entries = Some(4);

    let html = render_html_with_context("audit", &[], &[], false, context);

    assert!(html.contains("<td>Policy missing evidence</td><td class=\"count\">4</td>"));
    assert!(html.contains("cargo-allow worklist --format json"));
    assert!(html.contains("add <code>--missing-evidence</code> to focus that queue"));
}

#[test]
fn html_audit_report_counts_weak_evidence_references_context() {
    let mut context = context("git_tracked");
    context.weak_evidence_references = Some(2);

    let html = render_html_with_context("audit", &[], &[], false, context);

    assert!(html.contains("<td>Weak evidence/link references</td><td class=\"count\">2</td>"));
    assert!(
        html.contains("cargo-allow worklist --item-kind weak_evidence_reference --format json")
    );
    assert!(html.contains("replace unstructured or unknown-prefix evidence/link references"));
}

#[test]
fn html_check_report_includes_policy_context_status_counts() {
    let mut context = context("git_tracked");
    context.baseline_debt_entries = Some(3);
    context.policy_missing_evidence_entries = Some(4);
    context.broken_evidence_links = Some(2);
    context.weak_evidence_references = Some(1);

    let html = render_html_with_context("check", &[], &[], true, context);

    assert!(html.contains("<code>policy_baseline_debt</code></td><td class=\"count\">3</td>"));
    assert!(html.contains("<code>policy_missing_evidence</code></td><td class=\"count\">4</td>"));
    assert!(html.contains("<code>broken_evidence_links</code></td><td class=\"count\">2</td>"));
    assert!(html.contains("<code>weak_evidence_references</code></td><td class=\"count\">1</td>"));
}

#[test]
fn html_check_report_includes_evidence_repair_queues() {
    let mut context = context("git_tracked");
    context.policy_missing_evidence_entries = Some(2);
    context.broken_evidence_links = Some(1);
    context.weak_evidence_references = Some(1);

    let html = render_html_with_context("check", &[], &[], true, context);

    assert!(html.contains("<h3>Evidence Repair Queues</h3>"));
    assert!(html.contains("cargo-allow worklist --item-kind broken_evidence_link --format json"));
    assert!(html.contains("cargo-allow worklist --missing-evidence --format json"));
    assert!(
        html.contains("cargo-allow worklist --item-kind weak_evidence_reference --format json")
    );
    assert!(!html.contains("<h2>Audit Review Queue</h2>"));
}

#[test]
fn html_audit_report_routes_evidence_repairs_even_with_review_queue() {
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        finding_index: None,
        message: "unreceipted source exception".to_string(),
        score: 0,
    }];
    let mut context = context("git_tracked");
    context.policy_missing_evidence_entries = Some(2);
    context.broken_evidence_links = Some(1);
    context.weak_evidence_references = Some(1);

    let html = render_html_with_context("audit", &[], &outcomes, false, context);

    assert!(
        html.contains("Recommended next step: review the queue below before tightening policy.")
    );
    assert!(html.contains("<h3>Evidence Repair Queues</h3>"));
    assert!(html.contains("cargo-allow worklist --item-kind broken_evidence_link --format json"));
    assert!(html.contains("cargo-allow worklist --missing-evidence --format json"));
    assert!(
        html.contains("cargo-allow worklist --item-kind weak_evidence_reference --format json")
    );
    assert!(html.contains("<h2>Audit Review Queue</h2>"));
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
