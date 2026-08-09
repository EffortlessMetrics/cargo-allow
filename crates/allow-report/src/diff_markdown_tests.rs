use super::*;
use crate::diff_row_test_support::empty_ledger_movement_summary;

#[test]
fn diff_analysis_markdown_shows_revisions_and_absent_head() {
    let text = render_diff_analysis_markdown(DiffAnalysisContext {
        result_class: "complete",
        base_revision: Some("origin/main"),
        head_revision: None,
        base_inventory_complete: true,
        base_scanner_complete: true,
        head_inventory_complete: true,
        head_scanner_complete: true,
        introduced: 1,
        retained: 2,
        removed: 0,
    });
    assert!(text.contains("Base revision: `origin/main`"));
    assert!(text.contains("Head revision: `<none>`"));
}

#[test]
fn diff_pr_summary_markdown_reports_net_posture() {
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

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &policy_changes);

    assert!(summary.contains("**Net posture:** `improved`"));
    let reviewer_action = summary
        .find("**Reviewer action:**")
        .unwrap_or_else(|| std::panic::panic_any("reviewer action should be rendered"));
    let signal_table = summary
        .find("| Signal | Count |")
        .unwrap_or_else(|| std::panic::panic_any("signal table should be rendered"));
    assert!(
        reviewer_action < signal_table,
        "reviewer action should precede the signal table"
    );
    assert!(summary.contains("| Current check failures | 0 |"));
    assert!(summary.contains("| Removed source findings | 1 |"));
    assert!(summary.contains("| Policy improvements | 1 |"));
    assert!(summary.contains("keep the narrower posture"));
    assert!(summary.contains("> Claim boundary: scanned source-tree/source syntax only;"));
    assert!(summary.contains("cargo-allow did not invoke Cargo metadata"));
    assert!(summary.contains("### Finding Improvements"));
    assert!(summary.contains("| `removed` | `resolved` | `panic` | `unwrap` | `src/lib.rs` |"));
    assert!(summary.contains("### Policy Improvements"));
    assert!(summary.contains("| `allow-0001` | `selector_precision_increased` |"));
    assert!(
        !summary.contains("### Policy Failures"),
        "improvement-only summaries should not create failure rows"
    );
    assert!(
        !summary.contains("### Policy Review Required"),
        "improvement-only summaries should not create review rows"
    );
}

#[test]
fn diff_pr_summary_markdown_reports_evidence_health_rows() {
    let summary = render_diff_pr_summary_markdown_with_evidence_health_counts(
        1,
        1,
        3,
        2,
        &[],
        &[],
        empty_ledger_movement_summary(),
    );

    assert!(summary.contains("**Net posture:** `worse`"));
    assert!(summary.contains("| Current check failures | 1 |"));
    assert!(summary.contains("| Broken evidence links | 1 |"));
    assert!(summary.contains("| Missing evidence | 3 |"));
    assert!(summary.contains("| Weak evidence/link references | 2 |"));
    assert!(summary.contains("**Evidence repair queues:**"));
    assert!(summary.contains("`cargo-allow worklist --broken-evidence --format json`"));
    assert!(summary.contains("`cargo-allow worklist --missing-evidence --format json`"));
    assert!(summary.contains("`cargo-allow worklist --weak-evidence --format json`"));
}

#[test]
fn diff_pr_summary_markdown_reports_structural_delta_rows() {
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

    let summary = render_diff_pr_summary_markdown(0, &[], &policy_changes);

    assert!(summary.contains("| Scope broadened | 1 |"));
    assert!(summary.contains("| Scope changed | 1 |"));
    assert!(summary.contains("| Scope narrowed | 1 |"));
    assert!(summary.contains("| Selector changed | 1 |"));
    assert!(summary.contains("| Selector precision decreased | 1 |"));
    assert!(summary.contains("| Selector precision increased | 1 |"));
}

#[test]
fn diff_pr_summary_markdown_reports_evidence_delta_rows() {
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

    let summary = render_diff_pr_summary_markdown(0, &[], &policy_changes);

    assert!(summary.contains("| Evidence added | 3 |"));
    assert!(summary.contains("| Weak evidence added | 1 |"));
    assert!(summary.contains("| Broken evidence added | 1 |"));
    assert!(summary.contains("| Evidence removed | 3 |"));
    assert!(summary.contains("| Evidence removal failures | 1 |"));
    assert!(summary.contains("| Evidence removal review items | 1 |"));
    assert!(summary.contains("| Evidence removal improvements | 1 |"));
    assert!(summary.contains("| Links added | 3 |"));
    assert!(summary.contains("| Weak links added | 1 |"));
    assert!(summary.contains("| Broken links added | 1 |"));
    assert!(summary.contains("| Links removed | 3 |"));
    assert!(summary.contains("| Link removal failures | 1 |"));
    assert!(summary.contains("| Link removal review items | 1 |"));
    assert!(summary.contains("| Link removal improvements | 1 |"));
}

#[test]
fn diff_posture_tables_escape_markdown_cells() {
    let mut identity = allow_core::StructuralIdentity::new("rust", "method|call");
    identity.callee = Some("unwrap`call".to_string());
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
        kind: "panic|custom",
        family: Some("unwrap`family"),
        path: "src/lib.rs",
        line: Some(12),
        column: Some(5),
        source_package: Some("parser|core"),
        identity: Some(&identity),
    }];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        movement: "retained",
        posture_delta: "worsened",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
        allow_id: "allow|0001",
        kind: "scope_broadened",
        message: "message with | pipe",
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

    let findings = render_diff_finding_changes_markdown(&finding_changes);
    let policy = render_diff_policy_changes_markdown(&policy_changes);

    assert!(findings.contains("panic\\|custom"));
    assert!(findings.contains("unwrap\\`family"));
    assert!(findings.contains("src/lib.rs:12:5"));
    assert!(findings.contains("parser\\|core"));
    assert!(findings.contains("ast_kind=method\\|call,callee=unwrap\\`call"));
    assert!(policy.contains("allow\\|0001"));
    assert!(policy.contains("message with \\| pipe"));
}

#[test]
fn diff_finding_markdown_groups_findings_by_change() {
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

    let markdown = render_diff_finding_changes_markdown(&finding_changes);

    let attention = markdown
        .find("### Finding Attention")
        .unwrap_or_else(|| std::panic::panic_any("expected attention section"));
    let improvements = markdown
        .find("### Finding Improvements")
        .unwrap_or_else(|| std::panic::panic_any("expected improvement section"));
    assert!(
        attention < improvements,
        "markdown finding sections should show attention before improvements"
    );
    assert!(markdown.contains(
        "| Change | Coverage Movement | Kind | Family | Path | Source Package | Identity |"
    ));
    assert!(
        markdown.contains("| `new` | `new` | `unsafe` | `unsafe_block` | `src/new.rs:7:3` | `runtime` | `ast_kind=unsafe_block,container=runtime_init,callee=dangerous_call` |")
    );
    assert!(markdown.contains("| `removed` | `resolved` | `panic` | `unwrap` | `src/old.rs` |"));
}

#[test]
fn diff_policy_markdown_groups_policy_changes_by_severity() {
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

    let markdown = render_diff_policy_changes_markdown(&policy_changes);

    let failures = markdown
        .find("### Policy Failures")
        .unwrap_or_else(|| std::panic::panic_any("expected failure section"));
    let review = markdown
        .find("### Policy Review Required")
        .unwrap_or_else(|| std::panic::panic_any("expected review section"));
    let improvements = markdown
        .find("### Policy Improvements")
        .unwrap_or_else(|| std::panic::panic_any("expected improvement section"));
    assert!(
        failures < review && review < improvements,
        "markdown policy sections should be ordered by reviewer severity"
    );
    assert!(markdown.contains("| `fail` | `allow-fail` | `scope_broadened` | `worsened` |"));
    assert!(markdown.contains("| `review` | `allow-review` | `expiry_extended` | `retained` |"));
    assert!(
        markdown.contains("| `improvement` | `allow-improved` | `evidence_added` | `retained` |")
    );
}

#[test]
fn diff_pr_summary_markdown_highlights_policy_review_required() {
    let removed = vec!["test:old-proof".to_string()];
    let policy_changes = vec![DiffPolicyChange {
        severity: "review",
        movement: "retained",
        posture_delta: "review_required",
        changed_in_diff: true,
        subject: None,
        ledger_id: None,
        lane: None,
        allow_id: "allow|0042",
        kind: "evidence_removed",
        message: "allow-0042 evidence removed from policy",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: Some(DiffEvidenceChange {
            field: "evidence",
            removed: &removed,
            added: &[],
        }),
        metadata: None,
        requirement: None,
        policy_status: None,
    }];

    let summary = render_diff_pr_summary_markdown(0, &[], &policy_changes);

    assert!(summary.contains("**Net posture:** `review-required`"));
    assert!(summary.contains("### Policy Review Required"));
    assert!(!summary.contains("### Policy Failures"));
    assert!(
        summary.contains("| Severity | Allow ID | Kind | Coverage Movement | Detail | Message |"),
        "policy review rows should include structured details"
    );
    assert!(summary.contains("| `review` | `allow\\|0042` | `evidence_removed` | `retained` |"));
    assert!(summary.contains("evidence.evidence: removed: test:old-proof; added: none"));
    assert!(summary.contains("allow-0042 evidence removed from policy"));
}

#[test]
fn diff_pr_summary_markdown_highlights_policy_failures_separately() {
    let policy_changes = vec![
        DiffPolicyChange {
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
        },
        DiffPolicyChange {
            severity: "review",
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-0002",
            kind: "expiry_extended",
            message: "allow-0002 expiry extended",
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

    let summary = render_diff_pr_summary_markdown(0, &[], &policy_changes);

    assert!(summary.contains("**Net posture:** `worse`"));
    assert!(summary.contains("### Policy Failures"));
    assert!(summary.contains("| `fail` | `allow-0001` | `scope_broadened` | `worsened` |"));
    assert!(summary.contains("### Policy Review Required"));
    assert!(summary.contains("| `review` | `allow-0002` | `expiry_extended` | `retained` |"));
}

#[test]
fn diff_pr_summary_markdown_highlights_new_findings() {
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

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &[]);

    assert!(summary.contains("**Net posture:** `review-required`"));
    assert!(summary.contains("### Finding Attention"));
    assert!(summary.contains(
        "| Change | Coverage Movement | Kind | Family | Path | Source Package | Identity |"
    ));
    assert!(summary.contains("| `new` | `new` | `panic` | `unwrap` | `src/lib.rs:12:5` | `parser` | `ast_kind=method_call,callee=unwrap` |"));
    assert!(
        !summary.contains("### Finding Improvements"),
        "new-only summaries should not create finding improvement rows"
    );
}

#[test]
fn diff_pr_summary_markdown_reports_omitted_finding_highlights() {
    let finding = DiffFindingChange {
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
    };
    let finding_changes = vec![finding; 9];

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &[]);

    assert!(summary.contains("1 additional new finding change omitted from this summary."));
}

#[test]
fn diff_pr_summary_markdown_reports_omitted_policy_highlights() {
    let policy = DiffPolicyChange {
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
    };
    let policy_changes = vec![policy; 10];

    let summary = render_diff_pr_summary_markdown(0, &[], &policy_changes);

    assert!(summary.contains("2 additional policy failures omitted from this summary."));
}

fn bare_policy_change(
    severity: &'static str,
    allow_id: &'static str,
    kind: &'static str,
) -> DiffPolicyChange<'static> {
    crate::diff_row_test_support::test_policy_change(severity, allow_id, kind)
}
