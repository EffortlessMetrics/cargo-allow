use super::*;

#[test]
fn diff_pr_summary_markdown_reports_net_posture() {
    let finding_changes = vec![DiffFindingChange {
        change: "removed",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
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
    let summary = render_diff_pr_summary_markdown_with_evidence_health_counts(1, 1, 3, 2, &[], &[]);

    assert!(summary.contains("**Net posture:** `worse`"));
    assert!(summary.contains("| Current check failures | 1 |"));
    assert!(summary.contains("| Broken evidence links | 1 |"));
    assert!(summary.contains("| Missing evidence | 3 |"));
    assert!(summary.contains("| Weak evidence/link references | 2 |"));
    assert!(summary.contains("**Evidence repair queues:**"));
    assert!(
        summary.contains("`cargo-allow worklist --item-kind broken_evidence_link --format json`")
    );
    assert!(summary.contains("`cargo-allow worklist --missing-evidence --format json`"));
    assert!(
        summary
            .contains("`cargo-allow worklist --item-kind weak_evidence_reference --format json`")
    );
}

#[test]
fn diff_pr_summary_markdown_reports_evidence_delta_rows() {
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

    let summary = render_diff_pr_summary_markdown(0, &[], &policy_changes);

    assert!(summary.contains("| Evidence added | 1 |"));
    assert!(summary.contains("| Evidence removed | 1 |"));
    assert!(summary.contains("| Links added | 1 |"));
    assert!(summary.contains("| Links removed | 1 |"));
}

#[test]
fn diff_posture_tables_escape_markdown_cells() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic|custom",
        family: Some("unwrap`family"),
        path: "src/lib.rs",
        source_package: Some("parser|core"),
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
    assert!(findings.contains("parser\\|core"));
    assert!(policy.contains("allow\\|0001"));
    assert!(policy.contains("message with \\| pipe"));
}

#[test]
fn diff_finding_markdown_groups_findings_by_change() {
    let finding_changes = vec![
        DiffFindingChange {
            change: "removed",
            key: "panic|unwrap|src/old.rs",
            kind: "panic",
            family: Some("unwrap"),
            path: "src/old.rs",
            source_package: None,
        },
        DiffFindingChange {
            change: "new",
            key: "unsafe|unsafe_block|src/new.rs",
            kind: "unsafe",
            family: Some("unsafe_block"),
            path: "src/new.rs",
            source_package: Some("runtime"),
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
    assert!(markdown.contains("| Change | Kind | Family | Path | Source Package |"));
    assert!(markdown.contains("| `new` | `unsafe` | `unsafe_block` | `src/new.rs` | `runtime` |"));
    assert!(markdown.contains("| `removed` | `panic` | `unwrap` | `src/old.rs` |"));
}

#[test]
fn diff_policy_markdown_groups_policy_changes_by_severity() {
    let policy_changes = vec![
        DiffPolicyChange {
            severity: "review",
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
    assert!(markdown.contains("| `fail` | `allow-fail` | `scope_broadened` |"));
    assert!(markdown.contains("| `review` | `allow-review` | `expiry_extended` |"));
    assert!(markdown.contains("| `improvement` | `allow-improved` | `evidence_added` |"));
}

#[test]
fn diff_pr_summary_markdown_highlights_policy_review_required() {
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
    assert!(summary.contains("### Policy Review Required"));
    assert!(!summary.contains("### Policy Failures"));
    assert!(
        summary.contains("| Severity | Allow ID | Kind | Detail | Message |"),
        "policy review rows should include structured details"
    );
    assert!(summary.contains("| `review` | `allow\\|0042` | `evidence_removed` |"));
    assert!(summary.contains("evidence.evidence: removed: test:old-proof; added: none"));
    assert!(summary.contains("allow-0042 evidence removed from policy"));
}

#[test]
fn diff_pr_summary_markdown_highlights_policy_failures_separately() {
    let policy_changes = vec![
        DiffPolicyChange {
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
        },
        DiffPolicyChange {
            severity: "review",
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
    assert!(summary.contains("| `fail` | `allow-0001` | `scope_broadened` |"));
    assert!(summary.contains("### Policy Review Required"));
    assert!(summary.contains("| `review` | `allow-0002` | `expiry_extended` |"));
}

#[test]
fn diff_pr_summary_markdown_highlights_new_findings() {
    let finding_changes = vec![DiffFindingChange {
        change: "new",
        key: "panic|unwrap|src/lib.rs",
        kind: "panic",
        family: Some("unwrap"),
        path: "src/lib.rs",
        source_package: Some("parser"),
    }];

    let summary = render_diff_pr_summary_markdown(0, &finding_changes, &[]);

    assert!(summary.contains("**Net posture:** `review-required`"));
    assert!(summary.contains("### Finding Attention"));
    assert!(summary.contains("| Change | Kind | Family | Path | Source Package |"));
    assert!(summary.contains("| `new` | `panic` | `unwrap` | `src/lib.rs` | `parser` |"));
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
        source_package: None,
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

    assert!(summary.contains("2 additional policy failures omitted from this summary."));
}
