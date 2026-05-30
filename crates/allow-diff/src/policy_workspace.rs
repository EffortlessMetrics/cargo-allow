use allow_core::WorkspaceConfig;
use std::collections::BTreeSet;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

pub(crate) fn workspace_policy_changes(
    base: &WorkspaceConfig,
    head: &WorkspaceConfig,
) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    changes.extend(list_changes(
        ListPolicy {
            policy_id: "workspace.ignored",
            added_kind: PolicyChangeKind::WorkspaceIgnoredAdded,
            added_severity: PolicyChangeSeverity::Fail,
            added_message: "added ignored source-tree scope",
            removed_kind: PolicyChangeKind::WorkspaceIgnoredRemoved,
            removed_severity: PolicyChangeSeverity::Improvement,
            removed_message: "removed ignored source-tree scope",
        },
        &base.ignored,
        &head.ignored,
    ));
    changes.extend(list_changes(
        ListPolicy {
            policy_id: "workspace.generated",
            added_kind: PolicyChangeKind::WorkspaceGeneratedAdded,
            added_severity: PolicyChangeSeverity::Review,
            added_message: "added generated source-tree scope",
            removed_kind: PolicyChangeKind::WorkspaceGeneratedRemoved,
            removed_severity: PolicyChangeSeverity::Improvement,
            removed_message: "removed generated source-tree scope",
        },
        &base.generated,
        &head.generated,
    ));
    changes
}

struct ListPolicy {
    policy_id: &'static str,
    added_kind: PolicyChangeKind,
    added_severity: PolicyChangeSeverity,
    added_message: &'static str,
    removed_kind: PolicyChangeKind,
    removed_severity: PolicyChangeSeverity,
    removed_message: &'static str,
}

fn list_changes(policy: ListPolicy, base: &[String], head: &[String]) -> Vec<PolicyChange> {
    let base_values = normalized_values(base);
    let head_values = normalized_values(head);
    let mut changes = Vec::new();
    for value in head_values.difference(&base_values) {
        changes.push(change(
            policy.policy_id,
            policy.added_kind,
            policy.added_severity,
            policy.added_message,
            value,
        ));
    }
    for value in base_values.difference(&head_values) {
        changes.push(change(
            policy.policy_id,
            policy.removed_kind,
            policy.removed_severity,
            policy.removed_message,
            value,
        ));
    }
    changes
}

fn normalized_values(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.trim().replace('\\', "/"))
        .filter(|value| !value.is_empty())
        .collect()
}

fn change(
    policy_id: &str,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
    value: &str,
) -> PolicyChange {
    PolicyChange {
        allow_id: policy_id.to_string(),
        kind,
        severity,
        message: format!("{policy_id} {message}: {value}"),
        selector_precision: None,
        scope: None,
        occurrence_limit: None,
    }
}
