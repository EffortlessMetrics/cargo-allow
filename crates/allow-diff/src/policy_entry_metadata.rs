use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_compare::{added_required_text, changed_required_text, removed_required_text};

pub(crate) fn metadata_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if baseline_debt_normalized(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::BaselineDebtNormalized,
            PolicyChangeSeverity::Fail,
            "baseline_debt classification changed to reviewed policy",
        ));
    }
    if baseline_debt_introduced(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::BaselineDebtIntroduced,
            PolicyChangeSeverity::Fail,
            "reviewed policy reclassified as baseline_debt",
        ));
    }
    if removed_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerRemoved,
            PolicyChangeSeverity::Fail,
            "owner removed",
        ));
    }
    if owner_unassigned(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerUnassigned,
            PolicyChangeSeverity::Fail,
            "owner changed to unowned",
        ));
    }
    if changed_required_text(&base.owner, &head.owner) && !owner_unassigned(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerChanged,
            PolicyChangeSeverity::Review,
            "owner changed",
        ));
    }
    if added_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerAdded,
            PolicyChangeSeverity::Improvement,
            "owner added",
        ));
    }
    if removed_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            PolicyChangeKind::ReasonRemoved,
            PolicyChangeSeverity::Fail,
            "reason removed",
        ));
    }
    if changed_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            PolicyChangeKind::ReasonChanged,
            PolicyChangeSeverity::Review,
            "reason changed",
        ));
    }
    if added_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            PolicyChangeKind::ReasonAdded,
            PolicyChangeSeverity::Improvement,
            "reason added",
        ));
    }
    if removed_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            PolicyChangeKind::ClassificationRemoved,
            PolicyChangeSeverity::Fail,
            "classification removed",
        ));
    }
    if changed_required_text(&base.classification, &head.classification)
        && !baseline_debt_normalized(base, head)
        && !baseline_debt_introduced(base, head)
    {
        changes.push(change(
            head,
            PolicyChangeKind::ClassificationChanged,
            PolicyChangeSeverity::Review,
            "classification changed",
        ));
    }
    if added_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            PolicyChangeKind::ClassificationAdded,
            PolicyChangeSeverity::Improvement,
            "classification added",
        ));
    }
    changes
}

fn baseline_debt_normalized(base: &AllowEntry, head: &AllowEntry) -> bool {
    base.classification == "baseline_debt"
        && !head.classification.trim().is_empty()
        && head.classification != "baseline_debt"
}

fn baseline_debt_introduced(base: &AllowEntry, head: &AllowEntry) -> bool {
    base.classification != "baseline_debt" && head.classification == "baseline_debt"
}

fn owner_unassigned(base: &AllowEntry, head: &AllowEntry) -> bool {
    let base_owner = base.owner.trim();
    !base_owner.is_empty() && base_owner != "unowned" && head.owner.trim() == "unowned"
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
        scope: None,
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
    }
}
