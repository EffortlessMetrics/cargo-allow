use allow_core::AllowEntry;

use crate::policy_change::{
    EvidenceChange, EvidenceChangeField, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
};
use crate::policy_compare::{added_values, removed_values};

pub(crate) fn evidence_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if removed_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            EvidenceChangeField::Evidence,
            removed_items(&base.evidence, &head.evidence),
            Vec::new(),
            PolicyChangeKind::EvidenceRemoved,
            PolicyChangeSeverity::Fail,
            "evidence removed",
        ));
    }
    if added_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            EvidenceChangeField::Evidence,
            Vec::new(),
            added_items(&base.evidence, &head.evidence),
            PolicyChangeKind::EvidenceAdded,
            PolicyChangeSeverity::Improvement,
            "evidence added",
        ));
    }
    if removed_values(&base.links, &head.links) {
        changes.push(change(
            head,
            EvidenceChangeField::Links,
            removed_items(&base.links, &head.links),
            Vec::new(),
            PolicyChangeKind::LinkRemoved,
            PolicyChangeSeverity::Review,
            "traceability link removed",
        ));
    }
    if added_values(&base.links, &head.links) {
        changes.push(change(
            head,
            EvidenceChangeField::Links,
            Vec::new(),
            added_items(&base.links, &head.links),
            PolicyChangeKind::LinkAdded,
            PolicyChangeSeverity::Improvement,
            "traceability link added",
        ));
    }
    changes
}

fn change(
    entry: &AllowEntry,
    field: EvidenceChangeField,
    removed: Vec<String>,
    added: Vec<String>,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
) -> PolicyChange {
    PolicyChange::new(
        entry.id.clone(),
        kind,
        severity,
        format!("{} {message}", entry.id),
    )
    .with_evidence(EvidenceChange {
        field,
        removed,
        added,
    })
}

fn removed_items(base: &[String], head: &[String]) -> Vec<String> {
    base.iter()
        .filter(|item| !head.iter().any(|head| head == *item))
        .cloned()
        .collect()
}

fn added_items(base: &[String], head: &[String]) -> Vec<String> {
    head.iter()
        .filter(|item| !base.iter().any(|base| base == *item))
        .cloned()
        .collect()
}
