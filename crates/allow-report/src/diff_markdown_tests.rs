use super::*;

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
    assert!(summary.contains("| Current check failures | 0 |"));
    assert!(summary.contains("| Removed source findings | 1 |"));
    assert!(summary.contains("| Policy improvements | 1 |"));
    assert!(summary.contains("keep the narrower posture"));
    assert!(summary.contains("> Claim boundary: scanned source-tree/source syntax only;"));
    assert!(summary.contains("cargo-allow did not invoke Cargo metadata"));
    assert!(summary.contains("### Finding Improvements"));
    assert!(summary.contains("| `removed` | `panic` | `unwrap` | `src/lib.rs` |"));
    assert!(summary.contains("### Policy Improvements"));
    assert!(summary.contains("| `allow-0001` | `selector_precision_increased` |"));
    assert!(
        !summary.contains("### Policy Attention"),
        "improvement-only summaries should not create attention rows"
    );
}

#[test]
fn diff_pr_summary_markdown_reports_evidence_health_rows() {
    let summary = render_diff_pr_summary_markdown_with_evidence_health(1, 1, 2, &[], &[]);

    assert!(summary.contains("**Net posture:** `worse`"));
    assert!(summary.contains("| Current check failures | 1 |"));
    assert!(summary.contains("| Broken evidence links | 1 |"));
    assert!(summary.contains("| Weak evidence references | 2 |"));
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
    assert!(policy.contains("allow\\|0001"));
    assert!(policy.contains("message with \\| pipe"));
}

#[test]
fn diff_pr_summary_markdown_highlights_policy_attention() {
    let removed = vec!["test:old-proof".to_string()];
    let policy_changes = vec![DiffPolicyChange {
        severity: "review",
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
    assert!(summary.contains("### Policy Attention"));
    assert!(
        summary.contains("| Severity | Allow ID | Kind | Detail | Message |"),
        "policy attention should include structured details"
    );
    assert!(summary.contains("| `review` | `allow\\|0042` | `evidence_removed` |"));
    assert!(summary.contains("evidence.evidence: removed: test:old-proof; added: none"));
    assert!(summary.contains("allow-0042 evidence removed from policy"));
}

#[test]
fn diff_pr_summary_markdown_highlights_new_findings() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
    }];

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &[]);

    assert!(summary.contains("**Net posture:** `review-required`"));
    assert!(summary.contains("### Finding Attention"));
    assert!(summary.contains("| `new` | `panic` | `unwrap` | `src/lib.rs` |"));
    assert!(
        !summary.contains("### Finding Improvements"),
        "new-only summaries should not create finding improvement rows"
    );
}

#[test]
fn diff_pr_summary_markdown_reports_omitted_finding_highlights() {
    let finding = DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
    };
    let finding_changes = vec![finding; 9];

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &[]);

    assert!(summary.contains("1 additional new finding change omitted from this summary."));
}

#[test]
fn diff_pr_summary_markdown_reports_omitted_policy_highlights() {
    let policy = DiffPolicyChange {
        severity: "fail",
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

    assert!(summary.contains("2 additional policy attention changes omitted from this summary."));
}
