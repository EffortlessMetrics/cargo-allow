use super::*;

#[test]
fn diff_json_renderer_appends_posture_extension() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        movement: "introduced",
        posture_delta: "review_required",
        changed_in_diff: true,
        subject: None,
        allow_id: None,
        ledger_id: None,
        lane: None,
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: None,
        column: None,
        source_package: None,
        identity: None,
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        movement: "retained",
        posture_delta: "worsened",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
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
            ledger_movement: DiffLedgerMovementSummary {
                movement: DiffMovementCounts {
                    introduced: 0,
                    retained: 0,
                    removed: 0,
                },
                posture_delta: DiffPostureDeltaCounts {
                    improved: 0,
                    worsened: 0,
                    review_required: 0,
                    unchanged: 0,
                },
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
    assert!(json.contains("\"coverage_movement\": \"new\""));
    assert!(json.contains("\"coverage_movement\": \"worsened\""));
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
                ledger_movement: DiffLedgerMovementSummary {
                    movement: DiffMovementCounts {
                        introduced: 0,
                        retained: 0,
                        removed: 0
                    },
                    posture_delta: DiffPostureDeltaCounts {
                        improved: 0,
                        worsened: 0,
                        review_required: 0,
                        unchanged: 0
                    },
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
                ledger_movement: DiffLedgerMovementSummary {
                    movement: DiffMovementCounts {
                        introduced: 0,
                        retained: 0,
                        removed: 0
                    },
                    posture_delta: DiffPostureDeltaCounts {
                        improved: 0,
                        worsened: 0,
                        review_required: 0,
                        unchanged: 0
                    },
                },
                finding_changes: &[],
                policy_changes: &[],
            },
        )
        .is_none()
    );
}

#[test]
fn diff_json_projection_includes_shared_analysis_context() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis = DiffAnalysisContext {
        result_class: "base_partial",
        base_revision: Some("origin/main"),
        head_revision: Some("HEAD"),
        base_inventory_complete: true,
        base_scanner_complete: false,
        head_inventory_complete: true,
        head_scanner_complete: true,
        introduced: 2,
        retained: 3,
        removed: 0,
    };
    let context = ReportContext {
        diff_analysis: Some(analysis),
        ..ReportContext::default()
    };
    let json = render_json_with_context_and_diff(
        "diff",
        &[],
        &[],
        true,
        context,
        DiffReport {
            net_posture: "unchanged",
            reviewer_action: "repair scanner evidence",
            summary: DiffPostureSummary {
                current_failures: 1,
                new_findings: 0,
                removed_findings: 0,
                policy_failures: 0,
                policy_review_items: 0,
                policy_improvements: 0,
            },
            ledger_movement: crate::diff_row_test_support::empty_ledger_movement_summary(),
            finding_changes: &[],
            policy_changes: &[],
        },
    );
    let value: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(
        value["diff"]["diff_analysis"]["result_class"],
        "base_partial"
    );
    assert_eq!(
        value["diff"]["diff_analysis"]["base_revision"],
        "origin/main"
    );
    assert_eq!(value["diff"]["diff_analysis"]["head_revision"], "HEAD");
    assert_eq!(value["diff"]["diff_analysis"]["removed"], 0);
    Ok(())
}

#[test]
fn diff_json_renderer_survives_a_brace_inside_a_string_value() {
    // #1852: a `}` inside a string value (e.g. a finding message ending in a
    // brace) made the old `strip_suffix('}')` text surgery strip the wrong
    // brace, producing invalid JSON. The rendered output must be valid JSON
    // (round-trips through serde_json) and must carry the diff field.
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        movement: "retained",
        posture_delta: "worsened",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
        allow_id: "allow-0001",
        kind: "scope_broadened",
        message: "allow-0001 selector scope broadened",
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
    // A report whose body contains a string value ending in `}`. The naive
    // suffix-strip would slice inside this string.
    let report_json = concat!(
        "{\n",
        "  \"schema_id\": \"cargo-allow.report.v1\",\n",
        "  \"command\": \"diff\",\n",
        "  \"findings\": [\n",
        "    {\"message\": \"validation block: { }\"}\n",
        "  ]\n",
        "}"
    );
    let rendered = render_diff_json_with_posture(
        report_json,
        DiffReport {
            net_posture: "worse",
            reviewer_action: "block until fixed",
            summary: DiffPostureSummary {
                current_failures: 1,
                new_findings: 0,
                removed_findings: 0,
                policy_failures: 1,
                policy_review_items: 0,
                policy_improvements: 0,
            },
            ledger_movement: DiffLedgerMovementSummary {
                movement: DiffMovementCounts {
                    introduced: 0,
                    retained: 0,
                    removed: 0,
                },
                posture_delta: DiffPostureDeltaCounts {
                    improved: 0,
                    worsened: 0,
                    review_required: 0,
                    unchanged: 0,
                },
            },
            finding_changes: &[],
            policy_changes: &policy_changes,
        },
    );
    let Some(json) = rendered else {
        return;
    };
    // The whole document must be valid JSON — this fails on the pre-fix
    // text-surgery output, which sliced inside the message string.
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|err| {
        std::panic::panic_any(format!("output is not valid JSON: {err}\n{json}"))
    });
    assert_eq!(
        parsed.get("command").and_then(serde_json::Value::as_str),
        Some("diff"),
        "top-level command preserved"
    );
    assert!(
        parsed.get("diff").is_some(),
        "diff field present after splice"
    );
    assert_eq!(
        parsed
            .pointer("/findings/0/message")
            .and_then(serde_json::Value::as_str),
        Some("validation block: { }"),
        "brace-containing message value preserved verbatim"
    );
}

#[test]
fn diff_json_report_renderer_matches_existing_posture_extension() {
    let finding_changes = vec![DiffFindingChange {
        change: "removed",
        movement: "removed",
        posture_delta: "improved",
        changed_in_diff: true,
        subject: None,
        allow_id: None,
        ledger_id: None,
        lane: None,
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: None,
        column: None,
        source_package: None,
        identity: None,
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "improvement",
        movement: "retained",
        posture_delta: "improved",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
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
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
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
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
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
fn diff_json_report_summary_includes_nonzero_structural_delta_counts() {
    let policy_changes = vec![
        DiffPolicyChange {
            severity: "fail",
            movement: "retained",
            posture_delta: "worsened",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-scope-broadened",
            kind: "scope_broadened",
            message: "allow-scope-broadened scope broadened",
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
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-scope-changed",
            kind: "scope_changed",
            message: "allow-scope-changed scope changed",
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
            movement: "retained",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-scope-narrowed",
            kind: "scope_narrowed",
            message: "allow-scope-narrowed scope narrowed",
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
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-selector-changed",
            kind: "selector_changed",
            message: "allow-selector-changed selector changed",
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
            movement: "retained",
            posture_delta: "worsened",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-selector-decreased",
            kind: "selector_precision_decreased",
            message: "allow-selector-decreased selector precision decreased",
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
            movement: "retained",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-selector-increased",
            kind: "selector_precision_increased",
            message: "allow-selector-increased selector precision increased",
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
        reviewer_action: "review structural deltas",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 2,
            policy_review_items: 2,
            policy_improvements: 2,
        },
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    };
    let rendered = crate::diff_json::render_diff_posture_json(report);

    assert!(rendered.contains("\"scope_broadened\": 1"));
    assert!(rendered.contains("\"scope_changed\": 1"));
    assert!(rendered.contains("\"scope_narrowed\": 1"));
    assert!(rendered.contains("\"selector_changed\": 1"));
    assert!(rendered.contains("\"selector_precision_decreased\": 1"));
    assert!(rendered.contains("\"selector_precision_increased\": 1"));
}

#[test]
fn diff_json_report_summary_includes_nonzero_evidence_delta_counts() {
    let policy_changes = vec![
        DiffPolicyChange {
            severity: "improvement",
            movement: "retained",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
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
            movement: "retained",
            posture_delta: "worsened",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-broken-added",
            kind: "evidence_added",
            message: "allow-broken-added broken local evidence added",
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
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-weak-added",
            kind: "evidence_added",
            message: "allow-weak-added weak evidence added",
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
            movement: "retained",
            posture_delta: "worsened",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
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
            severity: "review",
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-removed-review",
            kind: "evidence_removed",
            message: "allow-removed-review weak evidence removed",
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
            movement: "retained",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-removed-improvement",
            kind: "evidence_removed",
            message: "allow-removed-improvement weak evidence removed",
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
            movement: "retained",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
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
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-weak-link-added",
            kind: "link_added",
            message: "allow-weak-link-added weak traceability link added",
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
            movement: "retained",
            posture_delta: "worsened",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-broken-link-added",
            kind: "link_added",
            message: "allow-broken-link-added broken local link added",
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
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
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
        DiffPolicyChange {
            severity: "fail",
            movement: "retained",
            posture_delta: "worsened",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-link-removed-fail",
            kind: "link_removed",
            message: "allow-link-removed-fail local traceability link removed",
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
            movement: "retained",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-link-removed-improvement",
            kind: "link_removed",
            message: "allow-link-removed-improvement weak traceability link removed",
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
            policy_failures: 4,
            policy_review_items: 4,
            policy_improvements: 4,
        },
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    };
    let rendered = crate::diff_json::render_diff_posture_json(report);

    assert!(rendered.contains("\"evidence_added\": 3"));
    assert!(rendered.contains("\"weak_evidence_added\": 1"));
    assert!(rendered.contains("\"broken_evidence_added\": 1"));
    assert!(rendered.contains("\"evidence_removed\": 3"));
    assert!(rendered.contains("\"evidence_removal_failures\": 1"));
    assert!(rendered.contains("\"evidence_removal_review_items\": 1"));
    assert!(rendered.contains("\"evidence_removal_improvements\": 1"));
    assert!(rendered.contains("\"link_added\": 3"));
    assert!(rendered.contains("\"weak_link_added\": 1"));
    assert!(rendered.contains("\"broken_link_added\": 1"));
    assert!(rendered.contains("\"link_removed\": 3"));
    assert!(rendered.contains("\"link_removal_failures\": 1"));
    assert!(rendered.contains("\"link_removal_review_items\": 1"));
    assert!(rendered.contains("\"link_removal_improvements\": 1"));
}

#[test]
fn diff_json_report_includes_finding_change_source_package_when_available() {
    let mut identity = allow_core::StructuralIdentity::new("rust", "method_call");
    identity.callee = Some("unwrap".to_string());
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        movement: "introduced",
        posture_delta: "review_required",
        changed_in_diff: true,
        subject: None,
        allow_id: None,
        ledger_id: None,
        lane: None,
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: Some(12),
        column: Some(5),
        source_package: Some("parser"),
        identity: Some(&identity),
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
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
        },
        finding_changes: &finding_changes,
        policy_changes: &[],
    };

    let rendered = crate::diff_json::render_diff_posture_json(report);

    assert!(rendered.contains("\"line\": 12"));
    assert!(rendered.contains("\"column\": 5"));
    assert!(rendered.contains("\"source_package\": \"parser\""));
    assert!(rendered.contains("\"identity\""));
    assert!(rendered.contains("\"callee\": \"unwrap\""));
}

#[test]
fn diff_json_report_matches_posture_golden_contract() {
    let finding_changes = vec![DiffFindingChange {
        change: "removed",
        movement: "removed",
        posture_delta: "improved",
        changed_in_diff: true,
        subject: None,
        allow_id: None,
        ledger_id: None,
        lane: None,
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        line: None,
        column: None,
        source_package: None,
        identity: None,
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "improvement",
        movement: "retained",
        posture_delta: "improved",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
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
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
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
  "rust_scanner": {{
    "completeness": "unknown",
    "files_considered": 0,
    "files_scanned": 0,
    "files_skipped": 0,
    "files_with_parse_errors": 0,
    "skipped_by_reason": {{
      "read_failed_or_unsupported": 0
    }}
  }},
  "summary": {{
    "findings": 0,
    "outcomes": 0,
    "matched": 0,
    "new": 0,
    "expired": 0,
    "review_due": 0,
    "location_drift": 0,
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
    "location_drift": 0,
    "stale": 0,
    "ambiguous": 0,
    "invalid_selector": 0,
    "missing_required_field": 0,
    "evidence_missing": 0,
    "baseline_debt": 0
  }},
  "evidence_repair_queues": [

  ],
  "outcomes": [

  ],
  "findings": [

  ],
  "diff": {{
    "net_posture": "improved",
    "reviewer_action": "keep narrower posture",
    "movement": {{
      "introduced": 0,
      "retained": 0,
      "removed": 0
    }},
    "posture_delta": {{
      "improved": 0,
      "worsened": 0,
      "review_required": 0,
      "unchanged": 0
    }},
    "summary": {{
      "current_failures": 0,
      "selector_precision_increased": 1,
      "new_findings": 0,
      "removed_findings": 1,
      "policy_failures": 0,
      "policy_review_items": 0,
      "policy_improvements": 1
    }},
    "finding_changes": [
      {{"change": "removed", "movement": "removed", "posture_delta": "improved", "changed_in_diff": true, "coverage_movement": "resolved", "key": "panic|unwrap|src/lib.rs", "kind": "panic", "family": "unwrap", "path": "src/lib.rs"}}
    ],
    "policy_changes": [
      {{"severity": "improvement", "movement": "retained", "posture_delta": "improved", "changed_in_diff": true, "coverage_movement": "retained", "allow_id": "allow-0001", "kind": "selector_precision_increased", "message": "allow-0001 selector precision increased", "selector_precision": {{"before": 42, "after": 92, "removed_fields": [], "added_fields": ["container", "normalized_snippet_hash"]}}}}
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
        ledger_movement: DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 0,
                retained: 0,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 0,
                review_required: 0,
                unchanged: 0,
            },
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
