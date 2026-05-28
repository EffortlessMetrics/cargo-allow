use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

fn context(source: &'static str) -> ReportContext<'static> {
    ReportContext {
        inventory_source: source,
        ..ReportContext::default()
    }
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
    assert!(html.contains("<h2>Non-Rust File Inventory</h2>"));
    assert!(html.contains("<code>new</code>"));
    assert!(html.contains("<code>scripts/new.sh</code>"));
    assert!(html.contains("did not invoke Cargo metadata"));
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
