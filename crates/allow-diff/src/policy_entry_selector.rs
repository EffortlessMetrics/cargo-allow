use allow_core::AllowEntry;

use crate::policy_change::{
    PolicyChange, PolicyChangeKind, PolicyChangeSeverity, SelectorIdentityChange,
    SelectorPrecisionChange,
};
use crate::policy_scope::{selector_precision_fields, selector_precision_score};
use crate::policy_selector::{selector_identity_changed, selector_identity_changed_fields};

pub(crate) fn selector_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let base_precision = selector_precision_score(base);
    let head_precision = selector_precision_score(head);
    if head_precision < base_precision {
        let selector_precision =
            selector_precision_change(base, head, base_precision, head_precision);
        vec![
            PolicyChange::new(
                head.id.clone(),
                PolicyChangeKind::SelectorPrecisionDecreased,
                PolicyChangeSeverity::Fail,
                format!(
                    "{} selector precision decreased: {} -> {}{}",
                    head.id,
                    base_precision,
                    head_precision,
                    selector_precision_detail(&selector_precision)
                ),
            )
            .with_selector_precision(selector_precision),
        ]
    } else if head_precision > base_precision {
        let selector_precision =
            selector_precision_change(base, head, base_precision, head_precision);
        vec![
            PolicyChange::new(
                head.id.clone(),
                PolicyChangeKind::SelectorPrecisionIncreased,
                PolicyChangeSeverity::Improvement,
                format!(
                    "{} selector precision increased: {} -> {}{}",
                    head.id,
                    base_precision,
                    head_precision,
                    selector_precision_detail(&selector_precision)
                ),
            )
            .with_selector_precision(selector_precision),
        ]
    } else if selector_identity_changed(&base.selector, &head.selector) {
        vec![
            change(
                head,
                PolicyChangeKind::SelectorChanged,
                PolicyChangeSeverity::Review,
                "selector identity changed",
            )
            .with_selector_identity(SelectorIdentityChange {
                changed_fields: selector_identity_changed_fields(&base.selector, &head.selector),
            }),
        ]
    } else {
        Vec::new()
    }
}

fn selector_precision_change(
    base: &AllowEntry,
    head: &AllowEntry,
    before: u32,
    after: u32,
) -> SelectorPrecisionChange {
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

    SelectorPrecisionChange {
        before,
        after,
        removed_fields: removed,
        added_fields: added,
    }
}

fn selector_precision_detail(change: &SelectorPrecisionChange) -> String {
    let mut details = Vec::new();
    if !change.removed_fields.is_empty() {
        details.push(format!("removed: {}", change.removed_fields.join(", ")));
    }
    if !change.added_fields.is_empty() {
        details.push(format!("added: {}", change.added_fields.join(", ")));
    }
    if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join("; "))
    }
}

fn change(
    entry: &AllowEntry,
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
}
