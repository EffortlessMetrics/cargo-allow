use crate::diff_row_test_support::empty_ledger_movement_summary;

use super::*;

#[test]
fn diff_finding_human_output_groups_findings_by_change() {
    let mut identity = allow_core::StructuralIdentity::new("rust", "unsafe_block");
    identity.container = Some("runtime_init".to_string());
    identity.callee = Some("dangerous_call".to_string());
    let finding_changes = vec![
        DiffFindingChange {
            change: "removed",
            movement: "removed",
            posture_delta: "improved",
            changed_in_diff: true,
            subject: None,
            allow_id: None,
            ledger_id: None,
            lane: None,
            key: "panic|unwrap|src/old.rs",
            kind: "panic",
            family: Some("unwrap"),
            path: "src/old.rs",
            line: None,
            column: None,
            source_package: None,
            identity: None,
        },
        DiffFindingChange {
            change: "new",
            movement: "introduced",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            allow_id: None,
            ledger_id: None,
            lane: None,
            key: "unsafe|unsafe_block|src/new.rs",
            kind: "unsafe",
            family: Some("unsafe_block"),
            path: "src/new.rs",
            line: Some(7),
            column: Some(3),
            source_package: Some("runtime"),
            identity: Some(&identity),
        },
    ];

    let text = render_diff_finding_changes_human(&finding_changes);

    let attention = text
        .find("Finding attention:")
        .unwrap_or_else(|| std::panic::panic_any("expected attention section"));
    let improvements = text
        .find("Finding improvements:")
        .unwrap_or_else(|| std::panic::panic_any("expected improvement section"));
    assert!(
        attention < improvements,
        "human finding sections should show attention before improvements"
    );
    assert!(text.contains("movement=introduced"));
    assert!(text.contains("unsafe.unsafe_block at src/new.rs:7:3 source_package=runtime"));
    assert!(
        text.contains(
            "identity=ast_kind=unsafe_block,container=runtime_init,callee=dangerous_call"
        )
    );
    assert!(text.contains("movement=removed"));
    assert!(text.contains("panic.unwrap at src/old.rs"));
    assert!(text.contains("coverage_movement=new"));
    assert!(text.contains("coverage_movement=resolved"));
}

#[test]
fn diff_policy_human_output_surfaces_coverage_movement_worsened() {
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

    let text = render_diff_policy_changes_human(&policy_changes);

    assert!(text.contains("coverage_movement=worsened"));
}

#[test]
fn diff_policy_human_output_includes_structured_details() {
    let removed_fields = ["container", "normalized_snippet_hash"];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        movement: "retained",
        posture_delta: "worsened",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
        allow_id: "allow-0001",
        kind: "selector_precision_decreased",
        message: "allow-0001 selector precision decreased",
        exception_identity: None,
        selector_identity: None,
        selector_precision: Some(DiffSelectorPrecisionChange {
            before: 82,
            after: 41,
            removed_fields: &removed_fields,
            added_fields: &[],
        }),
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
    }];

    let text = render_diff_policy_changes_human(&policy_changes);

    assert!(text.contains("Policy failures:"));
    assert!(text.contains("movement=retained"));
    assert!(text.contains("selector_precision_decreased"));
    assert!(text.contains(
        "detail: selector_precision: 82 -> 41; removed: container, normalized_snippet_hash; added: none"
    ));
}

#[test]
fn diff_policy_human_output_groups_policy_changes_by_severity() {
    let policy_changes = vec![
        DiffPolicyChange {
            severity: "review",
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-review",
            kind: "expiry_extended",
            message: "allow-review expiry extended",
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
            allow_id: "allow-improved",
            kind: "evidence_added",
            message: "allow-improved evidence added",
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
            allow_id: "allow-fail",
            kind: "scope_broadened",
            message: "allow-fail scope broadened",
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

    let text = render_diff_policy_changes_human(&policy_changes);

    let failures = text
        .find("Policy failures:")
        .unwrap_or_else(|| std::panic::panic_any("expected failure section"));
    let review = text
        .find("Policy review required:")
        .unwrap_or_else(|| std::panic::panic_any("expected review section"));
    let improvements = text
        .find("Policy improvements:")
        .unwrap_or_else(|| std::panic::panic_any("expected improvement section"));
    assert!(
        failures < review && review < improvements,
        "human policy sections should be ordered by reviewer severity"
    );
    assert!(text.contains("movement=retained"));
    assert!(text.contains("allow-fail scope_broadened"));
    assert!(text.contains("allow-review expiry_extended"));
    assert!(text.contains("allow-improved evidence_added"));
}

#[test]
fn diff_posture_human_summary_reports_reviewer_action() {
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
        message: "allow-0001 scope broadened",
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

    let text = render_diff_posture_summary_human(0, &finding_changes, &policy_changes);

    assert!(text.contains("Diff posture summary:"));
    assert!(text.contains("net_posture: worse"));
    assert!(text.contains(
        "reviewer_action: block until failing source exception changes are fixed, narrowed, or receipted."
    ));
    assert!(text.contains("new_source_findings: 1"));
    assert!(text.contains("policy_failures: 1"));

    let styled =
        render_diff_posture_summary_human_styled(0, &finding_changes, &policy_changes, Style::ANSI);
    assert!(styled.contains("net_posture: \u{1b}[31mworse\u{1b}[0m"));

    let finding_styled = render_diff_finding_changes_human_styled(&finding_changes, Style::ANSI);
    assert!(finding_styled.contains("\u{1b}[31mnew\u{1b}[0m"));

    let policy_styled = render_diff_policy_changes_human_styled(&policy_changes, Style::ANSI);
    assert!(policy_styled.contains("\u{1b}[31mfail\u{1b}[0m"));
    assert!(
        !policy_styled.contains("allow-0001\u{1b}"),
        "repository-controlled IDs must stay unstyled"
    );
    assert!(
        !policy_styled.contains("scope broadened\u{1b}"),
        "repository-controlled messages must stay unstyled"
    );
}

#[test]
fn diff_posture_human_summary_reports_evidence_health_counts() {
    let text = render_diff_posture_summary_human_with_evidence_health_counts(
        0,
        1,
        3,
        2,
        &[],
        &[],
        empty_ledger_movement_summary(),
    );

    assert!(text.contains("Diff posture summary:"));
    assert!(text.contains("broken_evidence_links: 1"));
    assert!(text.contains("missing_evidence: 3"));
    assert!(text.contains("weak_evidence_references: 2"));
    assert!(text.contains("evidence_repair_queues:"));
    assert!(text.contains("cargo-allow worklist --broken-evidence --format json"));
    assert!(text.contains("cargo-allow worklist --missing-evidence --format json"));
    assert!(text.contains("cargo-allow worklist --weak-evidence --format json"));
    assert!(text.contains("new_source_findings: 0"));
}

#[test]
fn diff_posture_human_summary_reports_structural_delta_counts() {
    let policy_changes = vec![
        bare_policy_change("fail", "allow-scope-broadened", "scope_broadened"),
        bare_policy_change("review", "allow-scope-changed", "scope_changed"),
        bare_policy_change("improvement", "allow-scope-narrowed", "scope_narrowed"),
        bare_policy_change("review", "allow-selector-changed", "selector_changed"),
        bare_policy_change(
            "fail",
            "allow-selector-decreased",
            "selector_precision_decreased",
        ),
        bare_policy_change(
            "improvement",
            "allow-selector-increased",
            "selector_precision_increased",
        ),
    ];

    let text = render_diff_posture_summary_human(0, &[], &policy_changes);

    assert!(text.contains("scope_broadened: 1"));
    assert!(text.contains("scope_changed: 1"));
    assert!(text.contains("scope_narrowed: 1"));
    assert!(text.contains("selector_changed: 1"));
    assert!(text.contains("selector_precision_decreased: 1"));
    assert!(text.contains("selector_precision_increased: 1"));
}

#[test]
fn diff_posture_human_summary_reports_evidence_delta_counts() {
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

    let text = render_diff_posture_summary_human(0, &[], &policy_changes);

    assert!(text.contains("evidence_added: 3"));
    assert!(text.contains("weak_evidence_added: 1"));
    assert!(text.contains("broken_evidence_added: 1"));
    assert!(text.contains("evidence_removed: 3"));
    assert!(text.contains("evidence_removal_failures: 1"));
    assert!(text.contains("evidence_removal_review_items: 1"));
    assert!(text.contains("evidence_removal_improvements: 1"));
    assert!(text.contains("link_added: 3"));
    assert!(text.contains("weak_link_added: 1"));
    assert!(text.contains("broken_link_added: 1"));
    assert!(text.contains("link_removed: 3"));
    assert!(text.contains("link_removal_failures: 1"));
    assert!(text.contains("link_removal_review_items: 1"));
    assert!(text.contains("link_removal_improvements: 1"));
}

#[test]
fn bare_policy_change_field_construction_observer() {
    let change = bare_policy_change("fail", "allow-test", "scope_broadened");
    assert_eq!(change.severity, "fail");
    assert_eq!(change.allow_id, "allow-test");
    assert_eq!(change.kind, "scope_broadened");
    assert_eq!(change.message, "policy changed");
    assert!(change.exception_identity.is_none());
    assert!(change.selector_identity.is_none());
    assert!(change.selector_precision.is_none());
    assert!(change.scope.is_none());
    assert!(change.occurrence_limit.is_none());
    assert!(change.lifecycle.is_none());
    assert!(change.evidence.is_none());
    assert!(change.metadata.is_none());
    assert!(change.requirement.is_none());
    assert!(change.policy_status.is_none());
}

fn bare_policy_change(
    severity: &'static str,
    allow_id: &'static str,
    kind: &'static str,
) -> DiffPolicyChange<'static> {
    crate::diff_row_test_support::test_policy_change(severity, allow_id, kind)
}
