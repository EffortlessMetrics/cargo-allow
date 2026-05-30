use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_compare::{date_extended, date_shortened, optional_text_added};
use crate::policy_compare::{optional_text_changed, optional_text_removed};

pub(crate) fn lifecycle_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
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
    }
}
