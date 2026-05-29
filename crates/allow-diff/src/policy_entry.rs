use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_compare::{
    added_required_text, added_values, date_extended, date_shortened, occurrence_limit_loosened,
    occurrence_limit_tightened, removed_required_text, removed_values,
};
use crate::policy_scope::{scope_broadened, scope_narrowed, selector_precision_score};

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
    let base_precision = selector_precision_score(base);
    let head_precision = selector_precision_score(head);
    if head_precision < base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionDecreased,
            severity: PolicyChangeSeverity::Fail,
            message: format!(
                "{} selector precision decreased: {} -> {}",
                head.id, base_precision, head_precision
            ),
        });
    } else if head_precision > base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionIncreased,
            severity: PolicyChangeSeverity::Improvement,
            message: format!(
                "{} selector precision increased: {} -> {}",
                head.id, base_precision, head_precision
            ),
        });
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
    if baseline_debt_normalized(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::BaselineDebtNormalized,
            PolicyChangeSeverity::Fail,
            "baseline_debt classification changed to reviewed policy",
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
