use allow_core::AllowConfig;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

pub(crate) fn policy_header_changes(base: &AllowConfig, head: &AllowConfig) -> Vec<PolicyChange> {
    let Some(change) = status_change(base.status.as_deref(), head.status.as_deref()) else {
        return Vec::new();
    };
    vec![change]
}

fn status_change(base: Option<&str>, head: Option<&str>) -> Option<PolicyChange> {
    let base = normalized_status(base);
    let head = normalized_status(head);
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
    Some(PolicyChange {
        allow_id: "policy.status".to_string(),
        kind,
        severity,
        message: format!(
            "policy.status {direction}: {} -> {}",
            display_status(base),
            display_status(head)
        ),
    })
}

fn normalized_status(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn display_status(value: Option<&str>) -> &str {
    value.unwrap_or("<unset>")
}
