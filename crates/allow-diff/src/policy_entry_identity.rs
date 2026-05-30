use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

pub(crate) fn identity_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if base.kind != head.kind {
        changes.push(PolicyChange::new(
            head.id.clone(),
            PolicyChangeKind::KindChanged,
            PolicyChangeSeverity::Fail,
            format!(
                "{} changed governed exception kind: {} -> {}",
                head.id,
                base.kind.as_str(),
                head.kind.as_str()
            ),
        ));
    }
    if base.family != head.family {
        changes.push(PolicyChange::new(
            head.id.clone(),
            PolicyChangeKind::FamilyChanged,
            PolicyChangeSeverity::Fail,
            format!(
                "{} changed governed exception family: {} -> {}",
                head.id,
                base.family.as_deref().unwrap_or("<none>"),
                head.family.as_deref().unwrap_or("<none>")
            ),
        ));
    }
    changes
}
