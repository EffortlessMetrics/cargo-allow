use allow_core::{Finding, MatchOutcome};
use allow_match::CheckMode;

use crate::OutputFormat;

pub(super) fn insert_markdown_pr_summary(text: &mut String, summary: &str) {
    allow_report::insert_markdown_pr_summary(text, summary);
}

pub(super) fn render_diff_pr_summary_markdown(
    current_failures: usize,
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let finding_rows = finding_change_rows(finding_changes);
    let policy_rows = policy_change_rows(policy_changes);
    allow_report::render_diff_pr_summary_markdown(
        current_failures.max(current_no_new_failures(outcomes)),
        &finding_rows,
        &policy_rows,
    )
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
