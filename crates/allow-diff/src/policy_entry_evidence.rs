use allow_core::AllowEntry;

use crate::policy_change::{
    EvidenceChange, EvidenceChangeField, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
};
use crate::policy_compare::{added_values, removed_values};

pub(crate) fn evidence_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if removed_values(&base.evidence, &head.evidence) {
        let removed = removed_items(&base.evidence, &head.evidence);
        let message = removed_evidence_message(&removed);
        changes.push(change(
            head,
            EvidenceChangeField::Evidence,
            removed,
            Vec::new(),
            PolicyChangeKind::EvidenceRemoved,
            PolicyChangeSeverity::Fail,
            message,
        ));
    }
    if added_values(&base.evidence, &head.evidence) {
        let added = added_items(&base.evidence, &head.evidence);
        let severity = added_evidence_severity(&added);
        changes.push(change(
            head,
            EvidenceChangeField::Evidence,
            Vec::new(),
            added,
            PolicyChangeKind::EvidenceAdded,
            severity,
            added_evidence_message(severity),
        ));
    }
    if removed_values(&base.links, &head.links) {
        let removed = removed_items(&base.links, &head.links);
        let severity = removed_link_severity(&removed);
        changes.push(change(
            head,
            EvidenceChangeField::Links,
            removed,
            Vec::new(),
            PolicyChangeKind::LinkRemoved,
            severity,
            removed_link_message(severity),
        ));
    }
    if added_values(&base.links, &head.links) {
        let added = added_items(&base.links, &head.links);
        let severity = added_link_severity(&added);
        changes.push(change(
            head,
            EvidenceChangeField::Links,
            Vec::new(),
            added,
            PolicyChangeKind::LinkAdded,
            severity,
            added_link_message(severity),
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

fn added_evidence_severity(added: &[String]) -> PolicyChangeSeverity {
    if added
        .iter()
        .any(|item| evidence_reference_is_invalid_local(item))
    {
        return PolicyChangeSeverity::Fail;
    }
    if added.iter().any(|item| evidence_reference_is_weak(item)) {
        PolicyChangeSeverity::Review
    } else {
        PolicyChangeSeverity::Improvement
    }
}

fn added_link_severity(added: &[String]) -> PolicyChangeSeverity {
    if added
        .iter()
        .any(|item| evidence_reference_is_invalid_local(item))
    {
        return PolicyChangeSeverity::Fail;
    }
    if added.iter().any(|item| reference_is_weak(item)) {
        PolicyChangeSeverity::Review
    } else {
        PolicyChangeSeverity::Improvement
    }
}

fn removed_link_severity(removed: &[String]) -> PolicyChangeSeverity {
    if removed.iter().any(|item| reference_is_local_file(item)) {
        PolicyChangeSeverity::Fail
    } else {
        PolicyChangeSeverity::Review
    }
}

fn added_evidence_message(severity: PolicyChangeSeverity) -> &'static str {
    match severity {
        PolicyChangeSeverity::Review => "weak evidence added",
        PolicyChangeSeverity::Improvement => "evidence added",
        PolicyChangeSeverity::Fail => "invalid local evidence added",
    }
}

fn added_link_message(severity: PolicyChangeSeverity) -> &'static str {
    match severity {
        PolicyChangeSeverity::Review => "weak traceability link added",
        PolicyChangeSeverity::Improvement => "traceability link added",
        PolicyChangeSeverity::Fail => "invalid traceability link added",
    }
}

fn removed_evidence_message(removed: &[String]) -> &'static str {
    if removed.iter().any(|item| reference_is_local_file(item)) {
        "local evidence removed"
    } else {
        "evidence removed"
    }
}

fn removed_link_message(severity: PolicyChangeSeverity) -> &'static str {
    match severity {
        PolicyChangeSeverity::Fail => "local traceability link removed",
        PolicyChangeSeverity::Review => "traceability link removed",
        PolicyChangeSeverity::Improvement => "traceability link removed",
    }
}

fn evidence_reference_is_invalid_local(reference: &str) -> bool {
    let Some((prefix, target)) = reference.split_once(':') else {
        return false;
    };
    if !reference_prefix_is_local_file(prefix) {
        return false;
    }
    let target = target.trim().replace('\\', "/");
    target.is_empty()
        || target.starts_with('/')
        || target.contains(':')
        || target.split('/').any(|part| part == "..")
        || target.chars().any(|ch| matches!(ch, '*' | '?'))
}

fn evidence_reference_is_weak(reference: &str) -> bool {
    reference_is_weak(reference)
}

fn reference_is_local_file(reference: &str) -> bool {
    reference
        .split_once(':')
        .map(|(prefix, _)| reference_prefix_is_local_file(prefix))
        .unwrap_or(false)
}

fn reference_prefix_is_local_file(prefix: &str) -> bool {
    allow_policy::local_file_evidence_prefixes().any(|known| known == prefix.trim())
}

fn reference_is_weak(reference: &str) -> bool {
    let Some((prefix, target)) = reference.split_once(':') else {
        return true;
    };
    let prefix = prefix.trim();
    let target = target.trim();
    target.is_empty() || !allow_policy::recognized_evidence_prefixes().any(|known| known == prefix)
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
