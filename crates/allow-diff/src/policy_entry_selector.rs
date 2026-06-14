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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector};
    use std::path::PathBuf;

    fn entry(id: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Range is validated before use.".to_string(),
            evidence: vec!["test:range_is_validated".to_string()],
            links: Vec::new(),
            occurrence_limit: Some(1),
            lifecycle: Lifecycle {
                created: Some("2026-05-26".to_string()),
                review_after: Some("2026-08-01".to_string()),
                expires: Some("2026-09-01".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                container: Some("load".to_string()),
                callee: Some("unwrap".to_string()),
                normalized_snippet_hash: Some("fnv1a64:1234".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn single_change(changes: &[PolicyChange]) -> &PolicyChange {
        match changes {
            [change] => change,
            _ => std::panic::panic_any(format!("expected one change, got {}", changes.len())),
        }
    }

    #[test]
    fn selector_policy_changes_reports_precision_decrease_detail() {
        let base = entry("allow-1");
        let mut head = entry("allow-1");
        head.selector.container = None;
        head.selector.normalized_snippet_hash = None;

        let changes = selector_policy_changes(&base, &head);

        let change = single_change(&changes);
        assert_eq!(change.kind, PolicyChangeKind::SelectorPrecisionDecreased);
        assert_eq!(change.severity, PolicyChangeSeverity::Fail);
        assert!(change.message.contains("selector precision decreased"));
        assert!(
            change
                .message
                .contains("removed: container, normalized_snippet_hash")
        );
        let detail = change
            .selector_precision
            .as_ref()
            .unwrap_or_else(|| std::panic::panic_any("missing precision detail"));
        assert!(detail.before > detail.after);
        assert_eq!(
            detail.removed_fields,
            vec!["container", "normalized_snippet_hash"]
        );
        assert!(detail.added_fields.is_empty());
    }

    #[test]
    fn selector_policy_changes_reports_precision_increase_detail() {
        let mut base = entry("allow-1");
        base.selector.container = None;
        base.selector.normalized_snippet_hash = None;
        let head = entry("allow-1");

        let changes = selector_policy_changes(&base, &head);

        let change = single_change(&changes);
        assert_eq!(change.kind, PolicyChangeKind::SelectorPrecisionIncreased);
        assert_eq!(change.severity, PolicyChangeSeverity::Improvement);
        assert!(change.message.contains("selector precision increased"));
        assert!(
            change
                .message
                .contains("added: container, normalized_snippet_hash")
        );
        let detail = change
            .selector_precision
            .as_ref()
            .unwrap_or_else(|| std::panic::panic_any("missing precision detail"));
        assert!(detail.after > detail.before);
        assert!(detail.removed_fields.is_empty());
        assert_eq!(
            detail.added_fields,
            vec!["container", "normalized_snippet_hash"]
        );
    }

    #[test]
    fn selector_policy_changes_reports_equal_precision_identity_change() {
        let base = entry("allow-1");
        let mut head = entry("allow-1");
        head.selector.container = Some("store".to_string());
        head.selector.normalized_snippet_hash = Some("fnv1a64:store".to_string());

        let changes = selector_policy_changes(&base, &head);

        let change = single_change(&changes);
        assert_eq!(change.kind, PolicyChangeKind::SelectorChanged);
        assert_eq!(change.severity, PolicyChangeSeverity::Review);
        assert_eq!(
            change
                .selector_identity
                .as_ref()
                .map(|identity| identity.changed_fields.clone()),
            Some(vec!["container", "normalized_snippet_hash"])
        );
        assert_eq!(change.selector_precision, None);
    }

    #[test]
    fn selector_policy_changes_returns_empty_when_selector_is_unchanged() {
        let base = entry("allow-1");
        let head = entry("allow-1");

        assert!(selector_policy_changes(&base, &head).is_empty());
    }

    #[test]
    fn selector_precision_change_tracks_added_and_removed_fields() {
        let mut base = entry("allow-1");
        let mut head = entry("allow-1");
        base.selector.callee = Some("unwrap".to_string());
        base.selector.macro_name = None;
        head.selector.callee = None;
        head.selector.macro_name = Some("panic".to_string());

        let change = selector_precision_change(&base, &head, 10, 10);

        assert_eq!(change.before, 10);
        assert_eq!(change.after, 10);
        assert_eq!(change.removed_fields, vec!["callee"]);
        assert_eq!(change.added_fields, vec!["macro_name"]);
    }

    #[test]
    fn selector_precision_detail_formats_removed_added_and_empty_details() {
        let empty = SelectorPrecisionChange {
            before: 1,
            after: 1,
            removed_fields: Vec::new(),
            added_fields: Vec::new(),
        };
        assert_eq!(selector_precision_detail(&empty), "");

        let changed = SelectorPrecisionChange {
            before: 10,
            after: 10,
            removed_fields: vec!["callee"],
            added_fields: vec!["macro_name"],
        };
        assert_eq!(
            selector_precision_detail(&changed),
            " (removed: callee; added: macro_name)"
        );
    }
}
