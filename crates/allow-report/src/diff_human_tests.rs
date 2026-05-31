use super::*;

#[test]
fn diff_policy_human_output_includes_structured_details() {
    let removed_fields = ["container", "normalized_snippet_hash"];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
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

    assert!(text.contains("fail allow-0001 selector_precision_decreased"));
    assert!(text.contains(
        "detail: selector_precision: 82 -> 41; removed: container, normalized_snippet_hash; added: none"
    ));
}

#[test]
fn diff_posture_human_summary_reports_reviewer_action() {
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
}
