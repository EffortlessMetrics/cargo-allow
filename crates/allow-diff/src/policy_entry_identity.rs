use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

pub(crate) fn identity_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if base.kind != head.kind {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::KindChanged,
            severity: PolicyChangeSeverity::Fail,
            message: format!(
                "{} changed governed exception kind: {} -> {}",
                head.id,
                base.kind.as_str(),
                head.kind.as_str()
            ),
            selector_precision: None,
            scope: None,
        });
    }
    if base.family != head.family {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::FamilyChanged,
            severity: PolicyChangeSeverity::Fail,
            message: format!(
                "{} changed governed exception family: {} -> {}",
                head.id,
                base.family.as_deref().unwrap_or("<none>"),
                head.family.as_deref().unwrap_or("<none>")
            ),
            selector_precision: None,
            scope: None,
        });
    }
    changes
}
