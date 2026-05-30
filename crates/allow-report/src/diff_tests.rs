use super::*;

#[test]
fn diff_json_renderer_appends_posture_extension() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow-0001",
        kind: "scope_broadened",
        message: "allow-0001 selector scope broadened",
    }];

    let rendered = render_diff_json_with_posture(
        "{\n  \"schema_id\": \"cargo-allow.report.v1\"\n}",
        DiffReport {
            net_posture: "worse",
            reviewer_action: "block until fixed",
            summary: DiffPostureSummary {
                current_failures: 1,
                new_findings: 1,
                removed_findings: 0,
                policy_failures: 1,
                policy_review_items: 0,
                policy_improvements: 0,
            },
            finding_changes: &finding_changes,
            policy_changes: &policy_changes,
        },
    );
    assert!(rendered.is_some());
    let Some(json) = rendered else {
        return;
    };

    assert!(json.contains("\"diff\""));
    assert!(json.contains("\"net_posture\": \"worse\""));
    assert!(json.contains("\"reviewer_action\": \"block until fixed\""));
    assert!(json.contains("\"current_failures\": 1"));
    assert!(json.contains("\"new_findings\": 1"));
    assert!(json.contains("\"policy_failures\": 1"));
    assert!(json.contains("\"change\": \"new\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"severity\": \"fail\""));
    assert!(json.contains("\"kind\": \"scope_broadened\""));
    assert!(json.ends_with("}\n"));
    assert!(
        render_diff_json_with_posture(
            "not json",
            DiffReport {
                net_posture: "unchanged",
                reviewer_action: "none",
                summary: DiffPostureSummary {
                    current_failures: 0,
                    new_findings: 0,
                    removed_findings: 0,
                    policy_failures: 0,
                    policy_review_items: 0,
                    policy_improvements: 0,
                },
                finding_changes: &[],
                policy_changes: &[],
            },
        )
        .is_none()
    );
}

#[test]
fn diff_json_report_renderer_matches_existing_posture_extension() {
    let finding_changes = vec![DiffFindingChange {
        change: "removed",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "improvement",
        allow_id: "allow-0001",
        kind: "selector_precision_increased",
        message: "allow-0001 selector precision increased",
    }];
    let report = DiffReport {
        net_posture: "improved",
        reviewer_action: "keep narrower posture",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 1,
            policy_failures: 0,
            policy_review_items: 0,
            policy_improvements: 1,
        },
        finding_changes: &finding_changes,
        policy_changes: &policy_changes,
    };
    let context = ReportContext::source_syntax("git_tracked", Some("H:/repo"), Some(2), None);
    let direct = render_json_with_context_and_diff("diff", &[], &[], false, context, report);

    assert!(direct.contains("\"schema_id\": \"cargo-allow.report.v1\""));
    assert!(direct.contains("\"command\": \"diff\""));
    assert!(direct.contains("\"status\": \"passed\""));
    assert!(direct.contains("\"source\": \"git_tracked\""));
    assert!(direct.contains("\"diff\": {"));
    assert!(direct.contains("\"net_posture\": \"improved\""));
    assert!(direct.contains("\"policy_improvements\": 1"));
    assert!(direct.contains("\"kind\": \"selector_precision_increased\""));
    assert!(direct.ends_with("}\n"));
}

#[test]
fn diff_pr_summary_markdown_reports_net_posture() {
    let finding_changes = vec![DiffFindingChange {
        change: "removed",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "improvement",
        allow_id: "allow-0001",
        kind: "selector_precision_increased",
        message: "allow-0001 selector precision increased",
    }];

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &policy_changes);

    assert!(summary.contains("**Net posture:** `improved`"));
    assert!(summary.contains("| Current check failures | 0 |"));
    assert!(summary.contains("| Removed source findings | 1 |"));
    assert!(summary.contains("| Policy improvements | 1 |"));
    assert!(summary.contains("keep the narrower posture"));
}

#[test]
fn diff_posture_tables_escape_markdown_cells() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic|custom",
        family: Some("unwrap`family"),
        path: "src/lib.rs",
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow|0001",
        kind: "scope_broadened",
        message: "message with | pipe",
    }];

    let findings = render_diff_finding_changes_markdown(&finding_changes);
    let policy = render_diff_policy_changes_markdown(&policy_changes);

    assert!(findings.contains("panic\\|custom"));
    assert!(findings.contains("unwrap\\`family"));
    assert!(policy.contains("allow\\|0001"));
    assert!(policy.contains("message with \\| pipe"));
}
