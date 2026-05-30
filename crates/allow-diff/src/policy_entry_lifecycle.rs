use allow_core::AllowEntry;

use crate::policy_change::{
    LifecycleChange, LifecycleChangeField, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
};
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
            LifecycleChangeField::Created,
            base.lifecycle.created.as_deref(),
            head.lifecycle.created.as_deref(),
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
            LifecycleChangeField::Created,
            base.lifecycle.created.as_deref(),
            head.lifecycle.created.as_deref(),
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
            LifecycleChangeField::Created,
            base.lifecycle.created.as_deref(),
            head.lifecycle.created.as_deref(),
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
            LifecycleChangeField::Expires,
            base.lifecycle.expires.as_deref(),
            head.lifecycle.expires.as_deref(),
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
            LifecycleChangeField::Expires,
            base.lifecycle.expires.as_deref(),
            head.lifecycle.expires.as_deref(),
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
            LifecycleChangeField::ReviewAfter,
            base.lifecycle.review_after.as_deref(),
            head.lifecycle.review_after.as_deref(),
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
            LifecycleChangeField::ReviewAfter,
            base.lifecycle.review_after.as_deref(),
            head.lifecycle.review_after.as_deref(),
            PolicyChangeKind::ReviewAfterShortened,
            PolicyChangeSeverity::Improvement,
            "review_after shortened or added",
        ));
    }
    changes
}

fn change(
    entry: &AllowEntry,
    field: LifecycleChangeField,
    before: Option<&str>,
    after: Option<&str>,
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
    .with_lifecycle(LifecycleChange {
        field,
        before: normalized_optional_text(before),
        after: normalized_optional_text(after),
    })
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
