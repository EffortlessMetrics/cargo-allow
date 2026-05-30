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
        selector_precision: None,
    }];

    let rendered = render_diff_json_with_posture(
        "{\n  \"schema_id\": \"cargo-allow.report.v1\",\n  \"command\": \"diff\"\n}",
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
    assert!(
        render_diff_json_with_posture(
            "{\n  \"schema_id\": \"cargo-allow.report.v1\",\n  \"command\": \"audit\"\n}",
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
        selector_precision: None,
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
fn diff_json_report_matches_posture_golden_contract() {
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
        selector_precision: Some(DiffSelectorPrecisionChange {
            before: 42,
            after: 92,
            removed_fields: &[],
            added_fields: &["container", "normalized_snippet_hash"],
        }),
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
    let json = render_json_with_context_and_diff("diff", &[], &[], false, context, report);
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.report.v1",
  "tool": "cargo-allow",
  "command": "diff",
  "status": "passed",
  "failed": false,
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/repo",
    "files_scanned": 2
  }},
  "summary": {{
    "findings": 0,
    "outcomes": 0,
    "matched": 0,
    "new": 0,
    "expired": 0,
    "review_due": 0,
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
    "stale": 0,
    "ambiguous": 0,
    "invalid_selector": 0,
    "missing_required_field": 0,
    "evidence_missing": 0,
    "baseline_debt": 0
  }},
  "outcomes": [

  ],
  "findings": [

  ],
  "diff": {{
    "net_posture": "improved",
    "reviewer_action": "keep narrower posture",
    "summary": {{
      "current_failures": 0,
      "new_findings": 0,
      "removed_findings": 1,
      "policy_failures": 0,
      "policy_review_items": 0,
      "policy_improvements": 1
    }},
    "finding_changes": [
      {{"change": "removed", "key": "panic|unwrap|src/lib.rs", "kind": "panic", "family": "unwrap", "path": "src/lib.rs"}}
    ],
    "policy_changes": [
      {{"severity": "improvement", "allow_id": "allow-0001", "kind": "selector_precision_increased", "message": "allow-0001 selector precision increased", "selector_precision": {{"before": 42, "after": 92, "removed_fields": [], "added_fields": ["container", "normalized_snippet_hash"]}}}}
    ]
  }}
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );

    assert_eq!(json, expected);
}

#[test]
#[should_panic(expected = "diff report artifacts support only diff command")]
fn diff_json_report_renderer_rejects_non_diff_command() {
    let report = DiffReport {
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
    };

    let _ = render_json_with_context_and_diff(
        "audit",
        &[],
        &[],
        false,
        ReportContext::default(),
        report,
    );
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
        selector_precision: None,
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
        selector_precision: None,
    }];

    let findings = render_diff_finding_changes_markdown(&finding_changes);
    let policy = render_diff_policy_changes_markdown(&policy_changes);

    assert!(findings.contains("panic\\|custom"));
    assert!(findings.contains("unwrap\\`family"));
    assert!(policy.contains("allow\\|0001"));
    assert!(policy.contains("message with \\| pipe"));
}
