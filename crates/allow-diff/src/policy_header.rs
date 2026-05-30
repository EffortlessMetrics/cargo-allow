use allow_core::AllowConfig;

use crate::policy_change::{
    MetadataChange, MetadataChangeField, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
    PolicyStatusChange,
};

pub(crate) fn policy_header_changes(base: &AllowConfig, head: &AllowConfig) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if let Some(change) = owner_change(base.owner.as_deref(), head.owner.as_deref()) {
        changes.push(change);
    }
    if let Some(change) = status_change(base.status.as_deref(), head.status.as_deref()) {
        changes.push(change);
    }
    changes
}

fn owner_change(base: Option<&str>, head: Option<&str>) -> Option<PolicyChange> {
    let base = normalized_text(base);
    let head = normalized_text(head);
    if base == head {
        return None;
    }
    let (kind, severity, direction) = match (base, head) {
        (Some(base), None) if base != "unowned" => (
            PolicyChangeKind::PolicyOwnerRemoved,
            PolicyChangeSeverity::Fail,
            "removed",
        ),
        (Some(base), Some("unowned")) if base != "unowned" => (
            PolicyChangeKind::PolicyOwnerUnassigned,
            PolicyChangeSeverity::Fail,
            "unassigned",
        ),
        (None | Some("unowned"), Some(head)) if head != "unowned" => (
            PolicyChangeKind::PolicyOwnerAdded,
            PolicyChangeSeverity::Improvement,
            "added",
        ),
        _ => (
            PolicyChangeKind::PolicyOwnerChanged,
            PolicyChangeSeverity::Review,
            "changed",
        ),
    };
    Some(
        PolicyChange::new(
            "policy.owner",
            kind,
            severity,
            format!(
                "policy.owner {direction}: {} -> {}",
                display_text(base),
                display_text(head)
            ),
        )
        .with_metadata(MetadataChange {
            field: MetadataChangeField::Owner,
            before: base.map(str::to_string),
            after: head.map(str::to_string),
        }),
    )
}

fn status_change(base: Option<&str>, head: Option<&str>) -> Option<PolicyChange> {
    let base = normalized_text(base);
    let head = normalized_text(head);
    if base == head {
        return None;
    }
    let (kind, severity, direction) = match (base, head) {
        (Some("active"), Some("advisory") | None) | (None, Some("advisory")) => (
            PolicyChangeKind::PolicyStatusWeakened,
            PolicyChangeSeverity::Fail,
            "weakened",
        ),
        (Some("advisory") | None, Some("active")) => (
            PolicyChangeKind::PolicyStatusTightened,
            PolicyChangeSeverity::Improvement,
            "tightened",
        ),
        _ => (
            PolicyChangeKind::PolicyStatusChanged,
            PolicyChangeSeverity::Review,
            "changed",
        ),
    };
    Some(
        PolicyChange::new(
            "policy.status",
            kind,
            severity,
            format!(
                "policy.status {direction}: {} -> {}",
                display_text(base),
                display_text(head)
            ),
        )
        .with_policy_status(PolicyStatusChange {
            before: base.map(str::to_string),
            after: head.map(str::to_string),
        }),
    )
}

fn normalized_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn display_text(value: Option<&str>) -> &str {
    value.unwrap_or("<unset>")
}
