use allow_core::{FileFamilyRule, WorkspaceConfig, normalize_path};
use std::collections::BTreeSet;

use crate::policy_change::{
    PolicyChange, PolicyChangeKind, PolicyChangeSeverity, ScopeChange, ScopeChangeField,
};

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
    changes.extend(file_family_rule_changes(
        &base.file_families,
        &head.file_families,
    ));
    changes
}

fn file_family_rule_changes(base: &[FileFamilyRule], head: &[FileFamilyRule]) -> Vec<PolicyChange> {
    let base_rules = base
        .iter()
        .map(|rule| (rule.id.as_str(), rule_signature(rule)))
        .collect::<BTreeSet<_>>();
    let head_rules = head
        .iter()
        .map(|rule| (rule.id.as_str(), rule_signature(rule)))
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();

    for (id, signature) in head_rules.difference(&base_rules) {
        changes.push(file_family_rule_change(
            id,
            PolicyChangeKind::FamilyRuleAdded,
            PolicyChangeSeverity::Review,
            "added repository file-family rule",
            None,
            Some(signature),
        ));
    }
    for (id, signature) in base_rules.difference(&head_rules) {
        changes.push(file_family_rule_change(
            id,
            PolicyChangeKind::FamilyRuleRemoved,
            PolicyChangeSeverity::Review,
            "removed repository file-family rule",
            Some(signature),
            None,
        ));
    }
    changes
}

fn rule_signature(rule: &FileFamilyRule) -> String {
    format!(
        "family={}; glob={}; reason={}",
        rule.family,
        normalize_path(std::path::Path::new(&rule.glob)),
        rule.reason.trim()
    )
}

fn file_family_rule_change(
    rule_id: &str,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
    before: Option<&String>,
    after: Option<&String>,
) -> PolicyChange {
    let allow_id = format!("workspace.file_family.{rule_id}");
    let value = after.or(before).map(String::as_str).unwrap_or("<unset>");
    PolicyChange::new(
        allow_id,
        kind,
        severity,
        format!("workspace file-family rule `{rule_id}` {message}: {value}"),
    )
    .with_scope(ScopeChange {
        field: ScopeChangeField::Effective,
        before: before.cloned(),
        after: after.cloned(),
    })
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
            None,
            Some(value),
        ));
    }
    for value in base_values.difference(&head_values) {
        changes.push(change(
            policy.policy_id,
            policy.removed_kind,
            policy.removed_severity,
            policy.removed_message,
            Some(value),
            None,
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
    before: Option<&str>,
    after: Option<&str>,
) -> PolicyChange {
    let value = after.or(before).unwrap_or("<unset>");
    PolicyChange::new(
        policy_id,
        kind,
        severity,
        format!("{policy_id} {message}: {value}"),
    )
    .with_scope(ScopeChange {
        field: ScopeChangeField::Effective,
        before: before.map(str::to_string),
        after: after.map(str::to_string),
    })
}
