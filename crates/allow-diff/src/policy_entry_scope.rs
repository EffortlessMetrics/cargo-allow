use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_scope::{scope_broadened, scope_changed, scope_narrowed};

pub(crate) fn scope_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if scope_broadened(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::ScopeBroadened,
            PolicyChangeSeverity::Fail,
            "scope broadened",
        ));
    }
    if scope_narrowed(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::ScopeNarrowed,
            PolicyChangeSeverity::Improvement,
            "scope narrowed",
        ));
    }
    if scope_changed(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::ScopeChanged,
            PolicyChangeSeverity::Review,
            "scope changed",
        ));
    }
    changes
}

fn change(
    entry: &AllowEntry,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
) -> PolicyChange {
    PolicyChange {
        allow_id: entry.id.clone(),
        kind,
        severity,
        message: format!("{} {message}", entry.id),
        selector_precision: None,
    }
}
