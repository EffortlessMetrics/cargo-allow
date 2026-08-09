use super::*;

fn empty_cfg() -> allow_core::AllowConfig {
    allow_core::AllowConfig::empty()
}

fn ledger<'a>(
    cfg: &'a allow_core::AllowConfig,
    finding_changes: &'a [allow_diff::FindingPostureChange],
    policy_changes: &'a [allow_diff::PolicyChange],
) -> DiffLedgerContext<'a> {
    DiffLedgerContext::new(
        cfg,
        cfg,
        finding_changes,
        policy_changes,
        allow_report::DiffAnalysisContext::default(),
    )
}

#[test]
fn markdown_pr_summary_reports_unchanged_posture() {
    let cfg = empty_cfg();
    let text = render_diff_pr_summary_markdown(
        0,
        EvidenceReportSummary::default(),
        &[],
        &ledger(&cfg, &[], &[]),
    );

    assert!(text.contains("**Net posture:** `unchanged`"));
    assert!(text.contains("| Current check failures | 0 |"));
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

    let cfg = empty_cfg();
    let text = render_diff_pr_summary_markdown(
        0,
        EvidenceReportSummary::default(),
        &[],
        &ledger(&cfg, &changes, &[]),
    );

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

    let cfg = empty_cfg();
    let text = render_diff_pr_summary_markdown(
        0,
        EvidenceReportSummary::default(),
        &[],
        &ledger(&cfg, &[], &changes),
    );

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

    let cfg = empty_cfg();
    let text = render_diff_pr_summary_markdown(
        0,
        EvidenceReportSummary::default(),
        &[],
        &ledger(&cfg, &changes, &[]),
    );

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

    let cfg = empty_cfg();
    let text = render_diff_pr_summary_markdown(
        0,
        EvidenceReportSummary::default(),
        &[],
        &ledger(&cfg, &[], &changes),
    );

    assert!(text.contains("**Net posture:** `improved`"));
    assert!(text.contains("| Policy improvements | 1 |"));
    assert!(text.contains("keep the narrower posture"));
}

#[test]
fn markdown_pr_summary_reports_evidence_health_counts() {
    let cfg = empty_cfg();
    let text = render_diff_pr_summary_markdown(
        1,
        EvidenceReportSummary {
            policy_missing_evidence_entries: 3,
            broken_evidence_links: 1,
            weak_evidence_references: 2,
            occurrence_headroom_entries: 0,
        },
        &[],
        &ledger(&cfg, &[], &[]),
    );

    assert!(text.contains("**Net posture:** `worse`"));
    assert!(text.contains("| Current check failures | 1 |"));
    assert!(text.contains("| Broken evidence links | 1 |"));
    assert!(text.contains("| Missing evidence | 3 |"));
    assert!(text.contains("| Weak evidence/link references | 2 |"));
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
        line: None,
        column: None,
        source_package: None,
        identity: allow_core::StructuralIdentity::new("rust", "method_call"),
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
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
    }
}
