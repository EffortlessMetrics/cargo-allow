use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_compare::{
    added_required_text, added_values, changed_required_text, date_extended, date_shortened,
    occurrence_limit_loosened, occurrence_limit_tightened, optional_text_added,
    optional_text_changed, optional_text_removed, removed_required_text, removed_values,
};
use crate::policy_scope::{
    scope_broadened, scope_changed, scope_narrowed, selector_precision_fields,
    selector_precision_score,
};
use crate::policy_selector::selector_identity_changed;

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
    append_selector_changes(&mut changes, base, head);
    if optional_text_removed(
        base.lifecycle.created.as_deref(),
        head.lifecycle.created.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::CreatedRemoved,
            PolicyChangeSeverity::Fail,
            "created date removed",
        ));
    }
    if optional_text_changed(
        base.lifecycle.created.as_deref(),
        head.lifecycle.created.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::CreatedChanged,
            PolicyChangeSeverity::Review,
            "created date changed",
        ));
    }
    if optional_text_added(
        base.lifecycle.created.as_deref(),
        head.lifecycle.created.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::CreatedAdded,
            PolicyChangeSeverity::Improvement,
            "created date added",
        ));
    }
    if date_extended(
        base.lifecycle.expires.as_deref(),
        head.lifecycle.expires.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ExpiryExtended,
            PolicyChangeSeverity::Review,
            "expiry extended or removed",
        ));
    }
    if date_shortened(
        base.lifecycle.expires.as_deref(),
        head.lifecycle.expires.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ExpiryShortened,
            PolicyChangeSeverity::Improvement,
            "expiry shortened or added",
        ));
    }
    if date_extended(
        base.lifecycle.review_after.as_deref(),
        head.lifecycle.review_after.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ReviewAfterExtended,
            PolicyChangeSeverity::Review,
            "review_after extended or removed",
        ));
    }
    if date_shortened(
        base.lifecycle.review_after.as_deref(),
        head.lifecycle.review_after.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ReviewAfterShortened,
            PolicyChangeSeverity::Improvement,
            "review_after shortened or added",
        ));
    }
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

fn append_selector_changes(changes: &mut Vec<PolicyChange>, base: &AllowEntry, head: &AllowEntry) {
    let base_precision = selector_precision_score(base);
    let head_precision = selector_precision_score(head);
    if head_precision < base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionDecreased,
            severity: PolicyChangeSeverity::Fail,
            message: format!(
                "{} selector precision decreased: {} -> {}{}",
                head.id,
                base_precision,
                head_precision,
                selector_precision_detail(base, head)
            ),
        });
    } else if head_precision > base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionIncreased,
            severity: PolicyChangeSeverity::Improvement,
            message: format!(
                "{} selector precision increased: {} -> {}{}",
                head.id,
                base_precision,
                head_precision,
                selector_precision_detail(base, head)
            ),
        });
    } else if selector_identity_changed(&base.selector, &head.selector) {
        changes.push(change(
            head,
            PolicyChangeKind::SelectorChanged,
            PolicyChangeSeverity::Review,
            "selector identity changed",
        ));
    }
}

fn selector_precision_detail(base: &AllowEntry, head: &AllowEntry) -> String {
    let base_fields = selector_precision_fields(base);
    let head_fields = selector_precision_fields(head);
    let mut removed = Vec::new();
    let mut added = Vec::new();

    for (base_field, head_field) in base_fields.into_iter().zip(head_fields) {
        match (base_field.present, head_field.present) {
            (true, false) => removed.push(base_field.label),
            (false, true) => added.push(head_field.label),
            _ => {}
        }
    }

    let mut details = Vec::new();
    if !removed.is_empty() {
        details.push(format!("removed: {}", removed.join(", ")));
    }
    if !added.is_empty() {
        details.push(format!("added: {}", added.join(", ")));
    }
    if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join("; "))
    }
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
