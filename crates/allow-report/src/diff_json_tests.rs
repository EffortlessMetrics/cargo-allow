use super::*;

#[test]
fn diff_json_renderer_appends_posture_extension() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: None,
        column: None,
        source_package: None,
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow-0001",
        kind: "scope_broadened",
        message: "allow-0001 selector scope broadened",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: Some(DiffScopeChange {
            field: "effective",
            before: Some("src/lib.rs"),
            after: Some("src/**"),
        }),
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
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
    assert!(json.contains(
        "\"scope\": {\"field\": \"effective\", \"before\": \"src/lib.rs\", \"after\": \"src/**\"}"
    ));
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
        line: None,
        column: None,
        source_package: None,
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "improvement",
        allow_id: "allow-0001",
        kind: "selector_precision_increased",
        message: "allow-0001 selector precision increased",
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
fn diff_json_report_summary_includes_nonzero_evidence_health() {
    let report = DiffReport {
        net_posture: "worse",
        reviewer_action: "repair evidence links",
        summary: DiffPostureSummary {
            current_failures: 2,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 0,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &[],
    };
    let rendered = crate::diff_json::render_diff_posture_json_with_evidence_health(report, 1, 3, 2);

    assert!(rendered.contains("\"broken_evidence_links\": 1"));
    assert!(rendered.contains("\"missing_evidence\": 3"));
    assert!(rendered.contains("\"weak_evidence_references\": 2"));
}

#[test]
fn diff_json_report_summary_includes_nonzero_evidence_delta_counts() {
    let policy_changes = vec![
        DiffPolicyChange {
            severity: "improvement",
            allow_id: "allow-added",
            kind: "evidence_added",
            message: "allow-added evidence added",
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
        },
        DiffPolicyChange {
            severity: "fail",
            allow_id: "allow-removed",
            kind: "evidence_removed",
            message: "allow-removed evidence removed",
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
        },
        DiffPolicyChange {
            severity: "improvement",
            allow_id: "allow-link-added",
            kind: "link_added",
            message: "allow-link-added link added",
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
        },
        DiffPolicyChange {
            severity: "review",
            allow_id: "allow-link-removed",
            kind: "link_removed",
            message: "allow-link-removed link removed",
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
        },
    ];
    let report = DiffReport {
        net_posture: "worse",
        reviewer_action: "review evidence deltas",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 1,
            policy_improvements: 2,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    };
    let rendered = crate::diff_json::render_diff_posture_json(report);

    assert!(rendered.contains("\"evidence_added\": 1"));
    assert!(rendered.contains("\"evidence_removed\": 1"));
    assert!(rendered.contains("\"link_added\": 1"));
    assert!(rendered.contains("\"link_removed\": 1"));
}

#[test]
fn diff_json_report_includes_finding_change_source_package_when_available() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: Some(12),
        column: Some(5),
        source_package: Some("parser"),
    }];
    let report = DiffReport {
        net_posture: "review-required",
        reviewer_action: "review new finding",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 1,
            removed_findings: 0,
            policy_failures: 0,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &finding_changes,
        policy_changes: &[],
    };

    let rendered = crate::diff_json::render_diff_posture_json(report);

    assert!(rendered.contains("\"line\": 12"));
    assert!(rendered.contains("\"column\": 5"));
    assert!(rendered.contains("\"source_package\": \"parser\""));
}

#[test]
fn diff_json_report_matches_posture_golden_contract() {
    let finding_changes = vec![DiffFindingChange {
        change: "removed",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: None,
        column: None,
        source_package: None,
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "improvement",
        allow_id: "allow-0001",
        kind: "selector_precision_increased",
        message: "allow-0001 selector precision increased",
        exception_identity: None,
        selector_identity: None,
        selector_precision: Some(DiffSelectorPrecisionChange {
            before: 42,
            after: 92,
            removed_fields: &[],
            added_fields: &["container", "normalized_snippet_hash"],
        }),
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
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
