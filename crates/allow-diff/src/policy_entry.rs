use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_compare::{
    added_required_text, changed_required_text, occurrence_limit_loosened,
    occurrence_limit_tightened, removed_required_text,
};
use crate::policy_entry_evidence::evidence_policy_changes;
use crate::policy_entry_lifecycle::lifecycle_policy_changes;
use crate::policy_entry_selector::selector_policy_changes;
use crate::policy_scope::{scope_broadened, scope_changed, scope_narrowed};

pub(crate) fn added_allow_change(entry: &AllowEntry) -> PolicyChange {
    let baseline = entry.classification == "baseline_debt";
    PolicyChange {
        allow_id: entry.id.clone(),
        kind: if baseline {
            PolicyChangeKind::BaselineDebtAdded
        } else {
            PolicyChangeKind::AddedAllow
        },
        severity: if baseline {
            PolicyChangeSeverity::Fail
        } else {
            PolicyChangeSeverity::Review
        },
        message: if baseline {
            format!("{} added generated baseline debt", entry.id)
        } else {
            format!("{} added a new allow entry", entry.id)
        },
    }
}

pub(crate) fn removed_allow_change(entry: &AllowEntry) -> PolicyChange {
    PolicyChange {
        allow_id: entry.id.clone(),
        kind: PolicyChangeKind::RemovedAllow,
        severity: PolicyChangeSeverity::Improvement,
        message: format!("{} removed an allow entry", entry.id),
    }
}

pub(crate) fn entry_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
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
        });
    }
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
    changes.extend(selector_policy_changes(base, head));
    changes.extend(lifecycle_policy_changes(base, head));
    changes.extend(evidence_policy_changes(base, head));
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
    if occurrence_limit_loosened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            head,
            PolicyChangeKind::OccurrenceLimitLoosened,
            PolicyChangeSeverity::Fail,
            "occurrence_limit increased or removed",
        ));
    }
    if occurrence_limit_tightened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            head,
            PolicyChangeKind::OccurrenceLimitTightened,
            PolicyChangeSeverity::Improvement,
            "occurrence_limit tightened",
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
    }
}
