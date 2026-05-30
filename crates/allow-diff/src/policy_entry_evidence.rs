use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_compare::{added_values, removed_values};

pub(crate) fn evidence_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if removed_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            PolicyChangeKind::EvidenceRemoved,
            PolicyChangeSeverity::Fail,
            "evidence removed",
        ));
    }
    if added_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            PolicyChangeKind::EvidenceAdded,
            PolicyChangeSeverity::Improvement,
            "evidence added",
        ));
    }
    if removed_values(&base.links, &head.links) {
        changes.push(change(
            head,
            PolicyChangeKind::LinkRemoved,
            PolicyChangeSeverity::Review,
            "traceability link removed",
        ));
    }
    if added_values(&base.links, &head.links) {
        changes.push(change(
            head,
            PolicyChangeKind::LinkAdded,
            PolicyChangeSeverity::Improvement,
            "traceability link added",
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
