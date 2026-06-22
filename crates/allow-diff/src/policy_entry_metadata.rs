use allow_core::AllowEntry;

use crate::policy_change::{
    MetadataChange, MetadataChangeField, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
};
use crate::policy_compare::{added_required_text, changed_required_text, removed_required_text};

pub(crate) fn metadata_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if baseline_debt_normalized(base, head) {
        changes.push(change(
            head,
            MetadataChangeField::Classification,
            Some(&base.classification),
            Some(&head.classification),
            PolicyChangeKind::BaselineDebtNormalized,
            PolicyChangeSeverity::Improvement,
            "baseline_debt classification changed to reviewed policy",
        ));
    }
    if baseline_debt_introduced(base, head) {
        changes.push(change(
            head,
            MetadataChangeField::Classification,
            Some(&base.classification),
            Some(&head.classification),
            PolicyChangeKind::BaselineDebtIntroduced,
            PolicyChangeSeverity::Fail,
            "reviewed policy reclassified as baseline_debt",
        ));
    }
    if removed_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            MetadataChangeField::Owner,
            Some(&base.owner),
            Some(&head.owner),
            PolicyChangeKind::OwnerRemoved,
            PolicyChangeSeverity::Fail,
            "owner removed",
        ));
    }
    if owner_unassigned(base, head) {
        changes.push(change(
            head,
            MetadataChangeField::Owner,
            Some(&base.owner),
            Some(&head.owner),
            PolicyChangeKind::OwnerUnassigned,
            PolicyChangeSeverity::Fail,
            "owner changed to unowned",
        ));
    }
    if changed_required_text(&base.owner, &head.owner) && !owner_unassigned(base, head) {
        changes.push(change(
            head,
            MetadataChangeField::Owner,
            Some(&base.owner),
            Some(&head.owner),
            PolicyChangeKind::OwnerChanged,
            PolicyChangeSeverity::Review,
            "owner changed",
        ));
    }
    if added_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            MetadataChangeField::Owner,
            Some(&base.owner),
            Some(&head.owner),
            PolicyChangeKind::OwnerAdded,
            PolicyChangeSeverity::Improvement,
            "owner added",
        ));
    }
    if removed_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            MetadataChangeField::Reason,
            Some(&base.reason),
            Some(&head.reason),
            PolicyChangeKind::ReasonRemoved,
            PolicyChangeSeverity::Fail,
            "reason removed",
        ));
    }
    if changed_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            MetadataChangeField::Reason,
            Some(&base.reason),
            Some(&head.reason),
            PolicyChangeKind::ReasonChanged,
            PolicyChangeSeverity::Review,
            "reason changed",
        ));
    }
    if added_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            MetadataChangeField::Reason,
            Some(&base.reason),
            Some(&head.reason),
            PolicyChangeKind::ReasonAdded,
            PolicyChangeSeverity::Improvement,
            "reason added",
        ));
    }
    if removed_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            MetadataChangeField::Classification,
            Some(&base.classification),
            Some(&head.classification),
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
            MetadataChangeField::Classification,
            Some(&base.classification),
            Some(&head.classification),
            PolicyChangeKind::ClassificationChanged,
            PolicyChangeSeverity::Review,
            "classification changed",
        ));
    }
    if added_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            MetadataChangeField::Classification,
            Some(&base.classification),
            Some(&head.classification),
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
    field: MetadataChangeField,
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
    .with_metadata(MetadataChange {
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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector};

    #[test]
    fn metadata_policy_changes_detects_baseline_debt_classification_transitions() {
        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "baseline_debt"),
                &entry("team", "reason", "reviewed_policy"),
            ),
            PolicyChangeKind::BaselineDebtNormalized,
            PolicyChangeSeverity::Improvement,
            MetadataChangeField::Classification,
            Some("baseline_debt"),
            Some("reviewed_policy"),
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "reviewed_policy"),
                &entry("team", "reason", "baseline_debt"),
            ),
            PolicyChangeKind::BaselineDebtIntroduced,
            PolicyChangeSeverity::Fail,
            MetadataChangeField::Classification,
            Some("reviewed_policy"),
            Some("baseline_debt"),
        );
    }

    #[test]
    fn metadata_policy_changes_detects_owner_transitions() {
        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "reviewed"),
                &entry(" ", "reason", "reviewed"),
            ),
            PolicyChangeKind::OwnerRemoved,
            PolicyChangeSeverity::Fail,
            MetadataChangeField::Owner,
            Some("team"),
            None,
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "reviewed"),
                &entry("unowned", "reason", "reviewed"),
            ),
            PolicyChangeKind::OwnerUnassigned,
            PolicyChangeSeverity::Fail,
            MetadataChangeField::Owner,
            Some("team"),
            Some("unowned"),
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "reviewed"),
                &entry("ops", "reason", "reviewed"),
            ),
            PolicyChangeKind::OwnerChanged,
            PolicyChangeSeverity::Review,
            MetadataChangeField::Owner,
            Some("team"),
            Some("ops"),
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry(" ", "reason", "reviewed"),
                &entry("team", "reason", "reviewed"),
            ),
            PolicyChangeKind::OwnerAdded,
            PolicyChangeSeverity::Improvement,
            MetadataChangeField::Owner,
            None,
            Some("team"),
        );
    }

    #[test]
    fn metadata_policy_changes_detects_reason_transitions() {
        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "old reason", "reviewed"),
                &entry("team", " ", "reviewed"),
            ),
            PolicyChangeKind::ReasonRemoved,
            PolicyChangeSeverity::Fail,
            MetadataChangeField::Reason,
            Some("old reason"),
            None,
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "old reason", "reviewed"),
                &entry("team", "new reason", "reviewed"),
            ),
            PolicyChangeKind::ReasonChanged,
            PolicyChangeSeverity::Review,
            MetadataChangeField::Reason,
            Some("old reason"),
            Some("new reason"),
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", " ", "reviewed"),
                &entry("team", "new reason", "reviewed"),
            ),
            PolicyChangeKind::ReasonAdded,
            PolicyChangeSeverity::Improvement,
            MetadataChangeField::Reason,
            None,
            Some("new reason"),
        );
    }

    #[test]
    fn metadata_policy_changes_detects_classification_transitions() {
        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "reviewed"),
                &entry("team", "reason", " "),
            ),
            PolicyChangeKind::ClassificationRemoved,
            PolicyChangeSeverity::Fail,
            MetadataChangeField::Classification,
            Some("reviewed"),
            None,
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", "reviewed"),
                &entry("team", "reason", "ffi_boundary"),
            ),
            PolicyChangeKind::ClassificationChanged,
            PolicyChangeSeverity::Review,
            MetadataChangeField::Classification,
            Some("reviewed"),
            Some("ffi_boundary"),
        );

        assert_metadata_change(
            metadata_policy_changes(
                &entry("team", "reason", " "),
                &entry("team", "reason", "reviewed"),
            ),
            PolicyChangeKind::ClassificationAdded,
            PolicyChangeSeverity::Improvement,
            MetadataChangeField::Classification,
            None,
            Some("reviewed"),
        );
    }

    #[test]
    fn normalized_optional_text_trims_empty_values() {
        assert_eq!(normalized_optional_text(None), None);
        assert_eq!(normalized_optional_text(Some("   ")), None);
        assert_eq!(
            normalized_optional_text(Some("  reviewed_policy  ")),
            Some("reviewed_policy".to_string())
        );
    }

    fn assert_metadata_change(
        changes: Vec<PolicyChange>,
        kind: PolicyChangeKind,
        severity: PolicyChangeSeverity,
        field: MetadataChangeField,
        before: Option<&str>,
        after: Option<&str>,
    ) {
        let change = match changes.as_slice() {
            [change] => change,
            other => std::panic::panic_any(format!("{other:#?}")),
        };
        assert_eq!(change.allow_id, "allow-test");
        assert_eq!(change.kind, kind);
        assert_eq!(change.severity, severity);
        assert!(change.message.contains("allow-test"));
        assert_eq!(
            change.metadata.as_ref().map(|metadata| metadata.field),
            Some(field)
        );
        assert_eq!(
            change
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.before.as_deref()),
            before
        );
        assert_eq!(
            change
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.after.as_deref()),
            after
        );
    }

    fn entry(owner: &str, reason: &str, classification: &str) -> AllowEntry {
        AllowEntry {
            id: "allow-test".to_string(),
            kind: FindingKind::Unsafe,
            family: Some("unsafe_block".to_string()),
            path: None,
            glob: Some("src/**/*.rs".to_string()),
            owner: owner.to_string(),
            classification: classification.to_string(),
            reason: reason.to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }
}
