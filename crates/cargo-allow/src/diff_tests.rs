use super::*;
use allow_core::{MatchOutcome, MatchStatus};

#[test]
fn markdown_pr_summary_reports_unchanged_posture() {
    let text = render_diff_pr_summary_markdown(&[], &[], &[]);

    assert!(text.contains("**Net posture:** `unchanged`"));
    assert!(text.contains("| Current no-new failures | 0 |"));
    assert!(text.contains("no source exception posture change detected"));
}

#[test]
fn markdown_pr_summary_reports_review_required_for_new_source_finding() {
    let changes = vec![finding_posture_change(
        allow_diff::FindingPostureKind::New,
        "panic",
        Some("unwrap"),
        "src/lib.rs",
    )];

    let text = render_diff_pr_summary_markdown(&[], &changes, &[]);

    assert!(text.contains("**Net posture:** `review-required`"));
    assert!(text.contains("| New source findings | 1 |"));
    assert!(text.contains("review the source exception posture change"));
}

#[test]
fn markdown_pr_summary_reports_worse_for_policy_failure() {
    let changes = vec![policy_change(
        allow_diff::PolicyChangeSeverity::Fail,
        allow_diff::PolicyChangeKind::ScopeBroadened,
    )];

    let text = render_diff_pr_summary_markdown(&[], &[], &changes);

    assert!(text.contains("**Net posture:** `worse`"));
    assert!(text.contains("| Policy failures | 1 |"));
    assert!(text.contains("block until failing source exception changes"));
}

#[test]
fn markdown_pr_summary_reports_improved_for_removed_source_finding() {
    let changes = vec![finding_posture_change(
        allow_diff::FindingPostureKind::Removed,
        "panic",
        Some("unwrap"),
        "src/lib.rs",
    )];

    let text = render_diff_pr_summary_markdown(&[], &changes, &[]);

    assert!(text.contains("**Net posture:** `improved`"));
    assert!(text.contains("| Removed source findings | 1 |"));
    assert!(text.contains("keep the narrower posture"));
}

#[test]
fn markdown_pr_summary_reports_improved_for_removed_policy_entry() {
    let changes = vec![policy_change(
        allow_diff::PolicyChangeSeverity::Improvement,
        allow_diff::PolicyChangeKind::RemovedAllow,
    )];

    let text = render_diff_pr_summary_markdown(&[], &[], &changes);

    assert!(text.contains("**Net posture:** `improved`"));
    assert!(text.contains("| Policy improvements | 1 |"));
    assert!(text.contains("keep the narrower posture"));
}

#[test]
fn json_report_includes_structured_posture_changes() {
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        None,
        Some(0),
        "unreceipted panic.unwrap at src/lib.rs:1:1",
    )];
    let finding_changes = vec![finding_posture_change(
        allow_diff::FindingPostureKind::New,
        "panic",
        Some("unwrap"),
        "src/lib.rs",
    )];
    let policy_changes = vec![policy_change(
        allow_diff::PolicyChangeSeverity::Fail,
        allow_diff::PolicyChangeKind::ScopeBroadened,
    )];

    let json = render_diff_json_with_posture(
        "{\n  \"schema_id\": \"cargo-allow.report.v1\"\n}".to_string(),
        &outcomes,
        &finding_changes,
        &policy_changes,
    );

    assert!(json.contains("\"diff\""));
    assert!(json.contains("\"net_posture\": \"worse\""));
    assert!(json.contains("\"current_failures\": 1"));
    assert!(json.contains("\"new_findings\": 1"));
    assert!(json.contains("\"policy_failures\": 1"));
    assert!(json.contains("\"policy_improvements\": 0"));
    assert!(json.contains("\"finding_changes\""));
    assert!(json.contains("\"change\": \"new\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"policy_changes\""));
    assert!(json.contains("\"severity\": \"fail\""));
    assert!(json.contains("\"kind\": \"scope_broadened\""));
    assert!(json.ends_with("}\n"));
}

#[test]
fn json_report_keeps_base_report_when_append_fails() {
    let base = "not json".to_string();

    let json = render_diff_json_with_posture(base.clone(), &[], &[], &[]);

    assert_eq!(json, base);
}

fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}

fn finding_posture_change(
    kind: allow_diff::FindingPostureKind,
    finding_kind: &str,
    family: Option<&str>,
    path: &str,
) -> allow_diff::FindingPostureChange {
    allow_diff::FindingPostureChange {
        kind,
        key: format!("{finding_kind}:{path}"),
        finding_kind: finding_kind.to_string(),
        family: family.map(str::to_string),
        path: path.to_string(),
    }
}

fn policy_change(
    severity: allow_diff::PolicyChangeSeverity,
    kind: allow_diff::PolicyChangeKind,
) -> allow_diff::PolicyChange {
    allow_diff::PolicyChange {
        allow_id: "allow-0001".to_string(),
        kind,
        severity,
        message: "allow-0001 changed".to_string(),
    }
}
