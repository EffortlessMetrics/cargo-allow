use allow_core::{Finding, MatchOutcome};
use allow_match::CheckMode;

use crate::OutputFormat;
use crate::reporting::EvidenceReportSummary;

pub(super) fn insert_markdown_pr_summary(text: &mut String, summary: &str) {
    allow_report::insert_markdown_pr_summary(text, summary);
}

pub(super) fn render_diff_pr_summary_markdown(
    current_failures: usize,
    evidence: EvidenceReportSummary,
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let finding_rows = finding_change_rows(finding_changes);
    let policy_rows = policy_change_rows(policy_changes);
    allow_report::render_diff_pr_summary_markdown_with_evidence_health_counts(
        current_failures.max(current_no_new_failures(outcomes)),
        evidence.broken_evidence_links,
        evidence.policy_missing_evidence_entries,
        evidence.weak_evidence_references,
        &finding_rows,
        &policy_rows,
    )
}

pub(super) fn append_diff_posture_summary(
    text: &mut String,
    format: OutputFormat,
    current_failures: usize,
    evidence: EvidenceReportSummary,
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) {
    if format != OutputFormat::Human {
        return;
    }
    let finding_rows = finding_change_rows(finding_changes);
    let policy_rows = policy_change_rows(policy_changes);
    text.push_str(
        &allow_report::render_diff_posture_summary_human_with_evidence_health_counts(
            current_failures.max(current_no_new_failures(outcomes)),
            evidence.broken_evidence_links,
            evidence.policy_missing_evidence_entries,
            evidence.weak_evidence_references,
            &finding_rows,
            &policy_rows,
        ),
    );
}

pub(super) fn append_finding_posture_changes(
    text: &mut String,
    format: OutputFormat,
    changes: &[allow_diff::FindingPostureChange],
) {
    let rows = finding_change_rows(changes);
    match format {
        OutputFormat::Human => {
            text.push_str(&allow_report::render_diff_finding_changes_human(&rows))
        }
        OutputFormat::Markdown => {
            text.push_str(&allow_report::render_diff_finding_changes_markdown(&rows));
        }
        OutputFormat::Html | OutputFormat::Json | OutputFormat::Sarif => {}
    }
}

#[cfg(test)]
pub(crate) fn render_diff_json_with_posture(
    report_json: String,
    current_failures: usize,
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let finding_rows = finding_change_rows(finding_changes);
    let policy_rows = policy_change_rows(policy_changes);
    let summary = allow_report::diff_posture_summary(
        current_failures.max(current_no_new_failures(outcomes)),
        &finding_rows,
        &policy_rows,
    );
    let posture = allow_report::diff_net_posture(summary);
    let report = allow_report::DiffReport {
        net_posture: posture.as_str(),
        reviewer_action: posture.reviewer_action(),
        summary,
        finding_changes: &finding_rows,
        policy_changes: &policy_rows,
    };
    if let Some(json) = allow_report::render_diff_json_with_posture(&report_json, report) {
        json
    } else {
        eprintln!("warning: failed to append diff posture to JSON report");
        report_json
    }
}

pub(crate) fn render_diff_json_report(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    report_context: allow_report::ReportContext<'_>,
    current_failures: usize,
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let finding_rows = finding_change_rows(finding_changes);
    let policy_rows = policy_change_rows(policy_changes);
    let summary = allow_report::diff_posture_summary(
        current_failures.max(current_no_new_failures(outcomes)),
        &finding_rows,
        &policy_rows,
    );
    let posture = allow_report::diff_net_posture(summary);
    let report = allow_report::DiffReport {
        net_posture: posture.as_str(),
        reviewer_action: posture.reviewer_action(),
        summary,
        finding_changes: &finding_rows,
        policy_changes: &policy_rows,
    };
    allow_report::render_json_with_context_and_diff(
        "diff",
        findings,
        outcomes,
        failed,
        report_context,
        report,
    )
}

pub(super) fn append_policy_changes(
    text: &mut String,
    format: OutputFormat,
    changes: &[allow_diff::PolicyChange],
) {
    let rows = policy_change_rows(changes);
    match format {
        OutputFormat::Human => {
            text.push_str(&allow_report::render_diff_policy_changes_human(&rows))
        }
        OutputFormat::Markdown => {
            text.push_str(&allow_report::render_diff_policy_changes_markdown(&rows));
        }
        OutputFormat::Html | OutputFormat::Json | OutputFormat::Sarif => {}
    }
}

pub(super) fn render_policy_changes_human(changes: &[allow_diff::PolicyChange]) -> String {
    let rows = policy_change_rows(changes);
    allow_report::render_diff_policy_changes_human(&rows)
}

pub(super) fn render_finding_posture_changes_human(
    changes: &[allow_diff::FindingPostureChange],
) -> String {
    let rows = finding_change_rows(changes);
    allow_report::render_diff_finding_changes_human(&rows)
}

fn current_no_new_failures(outcomes: &[MatchOutcome]) -> usize {
    outcomes
        .iter()
        .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
        .count()
}

fn finding_change_rows(
    changes: &[allow_diff::FindingPostureChange],
) -> Vec<allow_report::DiffFindingChange<'_>> {
    changes
        .iter()
        .map(|change| allow_report::DiffFindingChange {
            change: change.kind.as_str(),
            key: &change.key,
            kind: &change.finding_kind,
            family: change.family.as_deref(),
            path: &change.path,
            line: change.line,
            column: change.column,
            source_package: change.source_package.as_deref(),
            identity: Some(&change.identity),
        })
        .collect()
}

fn policy_change_rows(
    changes: &[allow_diff::PolicyChange],
) -> Vec<allow_report::DiffPolicyChange<'_>> {
    changes
        .iter()
        .map(|change| allow_report::DiffPolicyChange {
            severity: change.severity.as_str(),
            allow_id: &change.allow_id,
            kind: change.kind.as_str(),
            message: &change.message,
            exception_identity: change.exception_identity.as_ref().map(|identity| {
                allow_report::DiffExceptionIdentityChange {
                    field: identity.field.as_str(),
                    before: identity.before.as_deref(),
                    after: identity.after.as_deref(),
                }
            }),
            selector_identity: change.selector_identity.as_ref().map(|identity| {
                allow_report::DiffSelectorIdentityChange {
                    changed_fields: &identity.changed_fields,
                }
            }),
            selector_precision: change.selector_precision.as_ref().map(|selector| {
                allow_report::DiffSelectorPrecisionChange {
                    before: selector.before,
                    after: selector.after,
                    removed_fields: &selector.removed_fields,
                    added_fields: &selector.added_fields,
                }
            }),
            scope: change
                .scope
                .as_ref()
                .map(|scope| allow_report::DiffScopeChange {
                    field: scope.field.as_str(),
                    before: scope.before.as_deref(),
                    after: scope.after.as_deref(),
                }),
            occurrence_limit: change.occurrence_limit.as_ref().map(|limit| {
                allow_report::DiffOccurrenceLimitChange {
                    before: limit.before,
                    after: limit.after,
                }
            }),
            lifecycle: change.lifecycle.as_ref().map(|lifecycle| {
                allow_report::DiffLifecycleChange {
                    field: lifecycle.field.as_str(),
                    before: lifecycle.before.as_deref(),
                    after: lifecycle.after.as_deref(),
                }
            }),
            evidence: change
                .evidence
                .as_ref()
                .map(|evidence| allow_report::DiffEvidenceChange {
                    field: evidence.field.as_str(),
                    removed: &evidence.removed,
                    added: &evidence.added,
                }),
            metadata: change
                .metadata
                .as_ref()
                .map(|metadata| allow_report::DiffMetadataChange {
                    field: metadata.field.as_str(),
                    before: metadata.before.as_deref(),
                    after: metadata.after.as_deref(),
                }),
            requirement: change.requirement.as_ref().map(|requirement| {
                allow_report::DiffRequirementChange {
                    field: requirement.field.as_str(),
                    before: requirement.before,
                    after: requirement.after,
                }
            }),
            policy_status: change.policy_status.as_ref().map(|policy_status| {
                allow_report::DiffPolicyStatusChange {
                    before: policy_status.before.as_deref(),
                    after: policy_status.after.as_deref(),
                }
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::MatchStatus;

    #[test]
    fn current_no_new_failures_counts_only_no_new_failures() {
        let outcomes = MatchStatus::ALL
            .iter()
            .copied()
            .map(test_outcome)
            .collect::<Vec<_>>();

        assert_eq!(current_no_new_failures(&outcomes), 6);
    }

    #[test]
    fn finding_change_rows_maps_finding_posture_fields() {
        let mut identity = allow_core::StructuralIdentity::new("rust", "method_call");
        identity.callee = Some("unwrap".to_string());
        let changes = vec![allow_diff::FindingPostureChange {
            kind: allow_diff::FindingPostureKind::New,
            key: "panic:src/lib.rs".to_string(),
            finding_kind: "panic".to_string(),
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".to_string(),
            line: Some(17),
            column: Some(9),
            source_package: Some("parser".to_string()),
            identity,
        }];

        let rows = finding_change_rows(&changes);

        let [row] = rows.as_slice() else {
            std::panic::panic_any(format!("expected one finding row, got {}", rows.len()));
        };
        assert_eq!(row.change, "new");
        assert_eq!(row.key, "panic:src/lib.rs");
        assert_eq!(row.kind, "panic");
        assert_eq!(row.family, Some("unwrap"));
        assert_eq!(row.path, "src/lib.rs");
        assert_eq!(row.line, Some(17));
        assert_eq!(row.column, Some(9));
        assert_eq!(row.source_package, Some("parser"));
        let Some(identity) = row.identity else {
            std::panic::panic_any("expected finding identity");
        };
        assert_eq!(identity.language, "rust");
        assert_eq!(identity.ast_kind, "method_call");
        assert_eq!(identity.callee.as_deref(), Some("unwrap"));
    }

    #[test]
    fn policy_change_rows_maps_nested_policy_change_details() {
        let changes = vec![allow_diff::PolicyChange {
            allow_id: "allow-0001".to_string(),
            kind: allow_diff::PolicyChangeKind::SelectorPrecisionDecreased,
            severity: allow_diff::PolicyChangeSeverity::Fail,
            message: "selector precision decreased".to_string(),
            exception_identity: Some(allow_diff::ExceptionIdentityChange {
                field: allow_diff::ExceptionIdentityChangeField::Kind,
                before: Some("panic".to_string()),
                after: Some("unsafe".to_string()),
            }),
            selector_identity: Some(allow_diff::SelectorIdentityChange {
                changed_fields: vec!["container", "normalized_snippet_hash"],
            }),
            selector_precision: Some(allow_diff::SelectorPrecisionChange {
                before: 90,
                after: 40,
                removed_fields: vec!["container"],
                added_fields: vec!["glob"],
            }),
            scope: Some(allow_diff::ScopeChange {
                field: allow_diff::ScopeChangeField::Effective,
                before: Some("src/lib.rs".to_string()),
                after: Some("src/**".to_string()),
            }),
            occurrence_limit: Some(allow_diff::OccurrenceLimitChange {
                before: Some(1),
                after: None,
            }),
            lifecycle: Some(allow_diff::LifecycleChange {
                field: allow_diff::LifecycleChangeField::Expires,
                before: Some("2026-09-01".to_string()),
                after: Some("2026-12-01".to_string()),
            }),
            evidence: Some(allow_diff::EvidenceChange {
                field: allow_diff::EvidenceChangeField::Evidence,
                removed: vec!["test:old-proof".to_string()],
                added: vec!["test:new-proof".to_string()],
            }),
            metadata: Some(allow_diff::MetadataChange {
                field: allow_diff::MetadataChangeField::Owner,
                before: Some("core".to_string()),
                after: Some("runtime".to_string()),
            }),
            requirement: Some(allow_diff::RequirementChange {
                field: allow_diff::RequirementChangeField::OwnerRequired,
                before: true,
                after: false,
            }),
            policy_status: Some(allow_diff::PolicyStatusChange {
                before: Some("active".to_string()),
                after: Some("advisory".to_string()),
            }),
        }];

        let rows = policy_change_rows(&changes);

        let [row] = rows.as_slice() else {
            std::panic::panic_any(format!("expected one policy row, got {}", rows.len()));
        };
        assert_eq!(row.severity, "fail");
        assert_eq!(row.allow_id, "allow-0001");
        assert_eq!(row.kind, "selector_precision_decreased");
        assert_eq!(row.message, "selector precision decreased");
        let Some(exception_identity) = row.exception_identity else {
            std::panic::panic_any("expected exception identity detail");
        };
        assert_eq!(exception_identity.field, "kind");
        assert_eq!(exception_identity.before, Some("panic"));
        assert_eq!(exception_identity.after, Some("unsafe"));
        let Some(selector_identity) = row.selector_identity else {
            std::panic::panic_any("expected selector identity detail");
        };
        assert_eq!(
            selector_identity.changed_fields,
            ["container", "normalized_snippet_hash"]
        );
        let Some(selector_precision) = row.selector_precision else {
            std::panic::panic_any("expected selector precision detail");
        };
        assert_eq!(selector_precision.before, 90);
        assert_eq!(selector_precision.after, 40);
        assert_eq!(selector_precision.removed_fields, ["container"]);
        assert_eq!(selector_precision.added_fields, ["glob"]);
        let Some(scope) = row.scope else {
            std::panic::panic_any("expected scope detail");
        };
        assert_eq!(scope.field, "effective");
        assert_eq!(scope.before, Some("src/lib.rs"));
        assert_eq!(scope.after, Some("src/**"));
        let Some(occurrence_limit) = row.occurrence_limit else {
            std::panic::panic_any("expected occurrence limit detail");
        };
        assert_eq!(occurrence_limit.before, Some(1));
        assert_eq!(occurrence_limit.after, None);
        let Some(lifecycle) = row.lifecycle else {
            std::panic::panic_any("expected lifecycle detail");
        };
        assert_eq!(lifecycle.field, "expires");
        assert_eq!(lifecycle.before, Some("2026-09-01"));
        assert_eq!(lifecycle.after, Some("2026-12-01"));
        let Some(evidence) = row.evidence else {
            std::panic::panic_any("expected evidence detail");
        };
        assert_eq!(evidence.field, "evidence");
        assert_eq!(evidence.removed, ["test:old-proof"]);
        assert_eq!(evidence.added, ["test:new-proof"]);
        let Some(metadata) = row.metadata else {
            std::panic::panic_any("expected metadata detail");
        };
        assert_eq!(metadata.field, "owner");
        assert_eq!(metadata.before, Some("core"));
        assert_eq!(metadata.after, Some("runtime"));
        let Some(requirement) = row.requirement else {
            std::panic::panic_any("expected requirement detail");
        };
        assert_eq!(requirement.field, "owner_required");
        assert!(requirement.before);
        assert!(!requirement.after);
        let Some(policy_status) = row.policy_status else {
            std::panic::panic_any("expected policy status detail");
        };
        assert_eq!(policy_status.before, Some("active"));
        assert_eq!(policy_status.after, Some("advisory"));
    }

    #[test]
    fn append_finding_posture_changes_renders_only_text_formats() {
        let changes = vec![finding_change(allow_diff::FindingPostureKind::New)];

        let mut human = String::from("prefix");
        append_finding_posture_changes(&mut human, OutputFormat::Human, &changes);
        assert!(human.contains("Finding posture changes"));
        assert!(human.contains("new panic.unwrap at src/lib.rs"));

        let mut markdown = String::from("prefix");
        append_finding_posture_changes(&mut markdown, OutputFormat::Markdown, &changes);
        assert!(markdown.contains("Finding Posture Changes"));
        assert!(markdown.contains("src/lib.rs"));

        for format in [OutputFormat::Html, OutputFormat::Json, OutputFormat::Sarif] {
            let mut unchanged = String::from("prefix");
            append_finding_posture_changes(&mut unchanged, format, &changes);
            assert_eq!(unchanged, "prefix");
        }
    }

    #[test]
    fn append_policy_changes_renders_only_text_formats() {
        let changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Fail,
            allow_diff::PolicyChangeKind::ScopeBroadened,
        )];

        let mut human = String::from("prefix");
        append_policy_changes(&mut human, OutputFormat::Human, &changes);
        assert!(human.contains("Policy posture changes"));
        assert!(human.contains("fail allow-0001 scope_broadened"));

        let mut markdown = String::from("prefix");
        append_policy_changes(&mut markdown, OutputFormat::Markdown, &changes);
        assert!(markdown.contains("Policy Posture Changes"));
        assert!(markdown.contains("allow-0001"));

        for format in [OutputFormat::Html, OutputFormat::Json, OutputFormat::Sarif] {
            let mut unchanged = String::from("prefix");
            append_policy_changes(&mut unchanged, format, &changes);
            assert_eq!(unchanged, "prefix");
        }
    }

    #[test]
    fn append_diff_posture_summary_only_renders_for_human() {
        let finding_changes = vec![finding_change(allow_diff::FindingPostureKind::New)];
        let policy_changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Fail,
            allow_diff::PolicyChangeKind::ScopeBroadened,
        )];
        let outcomes = vec![test_outcome(MatchStatus::New)];

        let mut human = String::from("prefix");
        append_diff_posture_summary(
            &mut human,
            OutputFormat::Human,
            0,
            EvidenceReportSummary::default(),
            &outcomes,
            &finding_changes,
            &policy_changes,
        );
        assert!(human.contains("Diff posture summary"));
        assert!(human.contains("current_check_failures: 1"));
        assert!(human.contains("policy_failures: 1"));

        for format in [
            OutputFormat::Html,
            OutputFormat::Json,
            OutputFormat::Markdown,
            OutputFormat::Sarif,
        ] {
            let mut unchanged = String::from("prefix");
            append_diff_posture_summary(
                &mut unchanged,
                format,
                0,
                EvidenceReportSummary::default(),
                &outcomes,
                &finding_changes,
                &policy_changes,
            );
            assert_eq!(unchanged, "prefix");
        }
    }

    #[test]
    fn insert_markdown_pr_summary_inserts_summary_text() {
        let mut text = String::from("# cargo-allow report\n\nFindings scanned: 1\nbody\n");

        insert_markdown_pr_summary(&mut text, "summary text");

        assert!(text.contains("summary text"));
        let Some(summary_index) = text.find("summary text") else {
            std::panic::panic_any("expected inserted summary text");
        };
        let Some(findings_index) = text.find("Findings scanned:") else {
            std::panic::panic_any("expected findings marker");
        };
        assert!(summary_index < findings_index);
    }

    #[test]
    fn render_human_helpers_emit_mapped_rows() {
        let finding_changes = vec![finding_change(allow_diff::FindingPostureKind::Removed)];
        let policy_changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Improvement,
            allow_diff::PolicyChangeKind::RemovedAllow,
        )];

        let finding_text = render_finding_posture_changes_human(&finding_changes);
        assert!(finding_text.contains("Finding improvements"));
        assert!(finding_text.contains("removed panic.unwrap"));

        let policy_text = render_policy_changes_human(&policy_changes);
        assert!(policy_text.contains("Policy improvements"));
        assert!(policy_text.contains("improvement allow-0001 removed_allow"));
    }

    fn test_outcome(status: MatchStatus) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: None,
            finding_index: None,
            message: status.as_str().to_string(),
            score: 100,
        }
    }

    fn finding_change(kind: allow_diff::FindingPostureKind) -> allow_diff::FindingPostureChange {
        let mut identity = allow_core::StructuralIdentity::new("rust", "method_call");
        identity.callee = Some("unwrap".to_string());
        allow_diff::FindingPostureChange {
            kind,
            key: "panic:src/lib.rs".to_string(),
            finding_kind: "panic".to_string(),
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".to_string(),
            line: None,
            column: None,
            source_package: None,
            identity,
        }
    }

    fn policy_change(
        severity: allow_diff::PolicyChangeSeverity,
        kind: allow_diff::PolicyChangeKind,
    ) -> allow_diff::PolicyChange {
        allow_diff::PolicyChange {
            allow_id: "allow-0001".to_string(),
            kind,
            severity,
            message: "allow-0001 changed".to_string(),
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
        }
    }
}
