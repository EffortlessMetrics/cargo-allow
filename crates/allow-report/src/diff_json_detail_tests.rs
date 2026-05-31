use super::*;
use crate::diff_json::render_diff_posture_json;

#[test]
fn diff_json_renderer_includes_occurrence_limit_change() {
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow-0001",
        kind: "occurrence_limit_loosened",
        message: "allow-0001 occurrence_limit increased or removed",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: Some(DiffOccurrenceLimitChange {
            before: Some(1),
            after: None,
        }),
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "worse",
        reviewer_action: "block until fixed",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"occurrence_limit_loosened\""));
    assert!(json.contains("\"occurrence_limit\": {\"before\": 1, \"after\": null}"));
}

#[test]
fn diff_json_renderer_includes_lifecycle_change() {
    let policy_changes = vec![DiffPolicyChange {
        severity: "review",
        allow_id: "allow-0001",
        kind: "expiry_extended",
        message: "allow-0001 expiry extended or removed",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: Some(DiffLifecycleChange {
            field: "expires",
            before: Some("2026-09-01"),
            after: Some("2026-12-01"),
        }),
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "review-required",
        reviewer_action: "review policy lifecycle",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 0,
            policy_review_items: 1,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"expiry_extended\""));
    assert!(json.contains(
        "\"lifecycle\": {\"field\": \"expires\", \"before\": \"2026-09-01\", \"after\": \"2026-12-01\"}"
    ));
}

#[test]
fn diff_json_renderer_includes_evidence_change() {
    let removed = vec!["test:old-proof".to_string()];
    let added = vec!["doc:docs/new-proof.md".to_string()];
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow-0001",
        kind: "evidence_removed",
        message: "allow-0001 evidence removed",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: Some(DiffEvidenceChange {
            field: "evidence",
            removed: &removed,
            added: &added,
        }),
        metadata: None,
        requirement: None,
        policy_status: None,
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "worse",
        reviewer_action: "block until fixed",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"evidence_removed\""));
    assert!(json.contains(
        "\"evidence\": {\"field\": \"evidence\", \"removed\": [\"test:old-proof\"], \"added\": [\"doc:docs/new-proof.md\"]}"
    ));
}

#[test]
fn diff_json_renderer_includes_metadata_change() {
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow-0001",
        kind: "owner_removed",
        message: "allow-0001 owner removed",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: Some(DiffMetadataChange {
            field: "owner",
            before: Some("core"),
            after: None,
        }),
        requirement: None,
        policy_status: None,
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "worse",
        reviewer_action: "block until fixed",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"owner_removed\""));
    assert!(
        json.contains(
            "\"metadata\": {\"field\": \"owner\", \"before\": \"core\", \"after\": null}"
        )
    );
}

#[test]
fn diff_json_renderer_includes_requirement_change() {
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "requirements.owner_required",
        kind: "requirement_loosened",
        message: "requirements.owner_required loosened: true -> false",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: Some(DiffRequirementChange {
            field: "owner_required",
            before: true,
            after: false,
        }),
        policy_status: None,
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "worse",
        reviewer_action: "block until fixed",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"requirement_loosened\""));
    assert!(json.contains(
        "\"requirement\": {\"field\": \"owner_required\", \"before\": true, \"after\": false}"
    ));
}

#[test]
fn diff_json_renderer_includes_policy_status_change() {
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "policy.status",
        kind: "policy_status_weakened",
        message: "policy.status weakened: active -> advisory",
        exception_identity: None,
        selector_identity: None,
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: Some(DiffPolicyStatusChange {
            before: Some("active"),
            after: Some("advisory"),
        }),
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "worse",
        reviewer_action: "block until fixed",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"policy_status_weakened\""));
    assert!(json.contains("\"policy_status\": {\"before\": \"active\", \"after\": \"advisory\"}"));
}

#[test]
fn diff_json_renderer_includes_exception_identity_change() {
    let policy_changes = vec![DiffPolicyChange {
        severity: "fail",
        allow_id: "allow-0001",
        kind: "kind_changed",
        message: "allow-0001 changed governed exception kind: panic -> unsafe",
        exception_identity: Some(DiffExceptionIdentityChange {
            field: "kind",
            before: Some("panic"),
            after: Some("unsafe"),
        }),
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

    let json = render_diff_posture_json(DiffReport {
        net_posture: "worse",
        reviewer_action: "block until fixed",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 1,
            policy_review_items: 0,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"kind_changed\""));
    assert!(json.contains(
        "\"exception_identity\": {\"field\": \"kind\", \"before\": \"panic\", \"after\": \"unsafe\"}"
    ));
}

#[test]
fn diff_json_renderer_includes_selector_identity_change() {
    let changed_fields = ["container", "normalized_snippet_hash"];
    let policy_changes = vec![DiffPolicyChange {
        severity: "review",
        allow_id: "allow-0001",
        kind: "selector_changed",
        message: "allow-0001 selector identity changed",
        exception_identity: None,
        selector_identity: Some(DiffSelectorIdentityChange {
            changed_fields: &changed_fields,
        }),
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
        metadata: None,
        requirement: None,
        policy_status: None,
    }];

    let json = render_diff_posture_json(DiffReport {
        net_posture: "review-required",
        reviewer_action: "review policy changes",
        summary: DiffPostureSummary {
            current_failures: 0,
            new_findings: 0,
            removed_findings: 0,
            policy_failures: 0,
            policy_review_items: 1,
            policy_improvements: 0,
        },
        finding_changes: &[],
        policy_changes: &policy_changes,
    });

    assert!(json.contains("\"kind\": \"selector_changed\""));
    assert!(json.contains(
        "\"selector_identity\": {\"changed_fields\": [\"container\", \"normalized_snippet_hash\"]}"
    ));
}
