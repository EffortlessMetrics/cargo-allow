use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_scope::{selector_precision_fields, selector_precision_score};
use crate::policy_selector::selector_identity_changed;

pub(crate) fn selector_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let base_precision = selector_precision_score(base);
    let head_precision = selector_precision_score(head);
    if head_precision < base_precision {
        vec![PolicyChange {
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
        }]
    } else if head_precision > base_precision {
        vec![PolicyChange {
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
        }]
    } else if selector_identity_changed(&base.selector, &head.selector) {
        vec![change(
            head,
            PolicyChangeKind::SelectorChanged,
            PolicyChangeSeverity::Review,
            "selector identity changed",
        )]
    } else {
        Vec::new()
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
