use super::*;

#[test]
fn detects_required_metadata_removed_and_limit_loosened() {
    let base = config_with(entry("allow-1"));
    let mut weaker = entry("allow-1");
    weaker.owner.clear();
    weaker.reason.clear();
    weaker.classification.clear();
    weaker.occurrence_limit = None;
    let head = config_with(weaker);

    let changes = policy_changes(&base, &head);

    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::OwnerRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ReasonRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ClassificationRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::OccurrenceLimitLoosened)
    );
}

#[test]
fn detects_required_metadata_changed_for_review() {
    let base = config_with(entry("allow-1"));
    let mut changed = entry("allow-1");
    changed.owner = "security".to_string();
    changed.reason = "Different retained exception rationale.".to_string();
    changed.classification = "different_review_bucket".to_string();
    let head = config_with(changed);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OwnerChanged
            && change.severity == PolicyChangeSeverity::Review
    }));
    let owner = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::OwnerChanged)
        .unwrap_or_else(|| std::panic::panic_any("owner change should be reported"));
    assert_eq!(
        owner.metadata.as_ref().map(|change| (
            change.field,
            change.before.as_deref(),
            change.after.as_deref()
        )),
        Some((MetadataChangeField::Owner, Some("core"), Some("security")))
    );
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReasonChanged
            && change.severity == PolicyChangeSeverity::Review
    }));
    let reason = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::ReasonChanged)
        .unwrap_or_else(|| std::panic::panic_any("reason change should be reported"));
    assert_eq!(
        reason.metadata.as_ref().map(|change| (
            change.field,
            change.before.as_deref(),
            change.after.as_deref()
        )),
        Some((
            MetadataChangeField::Reason,
            Some("Range is validated before use."),
            Some("Different retained exception rationale.")
        ))
    );
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ClassificationChanged
            && change.severity == PolicyChangeSeverity::Review
    }));
    let classification = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::ClassificationChanged)
        .unwrap_or_else(|| std::panic::panic_any("classification change should be reported"));
    assert_eq!(
        classification.metadata.as_ref().map(|change| (
            change.field,
            change.before.as_deref(),
            change.after.as_deref()
        )),
        Some((
            MetadataChangeField::Classification,
            Some("reviewed_exception"),
            Some("different_review_bucket")
        ))
    );
}

#[test]
fn detects_owner_unassigned_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head_entry = entry("allow-1");
    head_entry.owner = "unowned".to_string();
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OwnerUnassigned
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("unowned")
    }));
    assert!(
        !changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::OwnerChanged),
        "changing a real owner to unowned should not be downgraded to a generic owner change"
    );
}

#[test]
fn detects_policy_status_weakened_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.status = Some("advisory".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyStatusWeakened
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "policy.status"
            && change.message.contains("active -> advisory")
    }));
}

#[test]
fn detects_policy_owner_removed_as_failure() {
    let mut base = config_with(entry("allow-1"));
    base.owner = Some("core/policy".to_string());
    let mut head = base.clone();
    head.owner = None;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyOwnerRemoved
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "policy.owner"
            && change.message.contains("core/policy -> <unset>")
    }));
}

#[test]
fn detects_policy_owner_unassigned_as_failure() {
    let mut base = config_with(entry("allow-1"));
    base.owner = Some("core/policy".to_string());
    let mut head = base.clone();
    head.owner = Some("unowned".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyOwnerUnassigned
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "policy.owner"
            && change.message.contains("core/policy -> unowned")
    }));
}

#[test]
fn detects_policy_owner_added_as_improvement() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.owner = Some("core/policy".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyOwnerAdded
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "policy.owner"
            && change.message.contains("<unset> -> core/policy")
    }));
}

#[test]
fn detects_policy_owner_changed_for_review() {
    let mut base = config_with(entry("allow-1"));
    base.owner = Some("core/policy".to_string());
    let mut head = base.clone();
    head.owner = Some("security".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyOwnerChanged
            && change.severity == PolicyChangeSeverity::Review
            && change.allow_id == "policy.owner"
            && change.message.contains("core/policy -> security")
    }));
}

#[test]
fn detects_policy_status_tightened_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.status = Some("advisory".to_string());
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyStatusTightened
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "policy.status"
            && change.message.contains("advisory -> active")
    }));
}

#[test]
fn detects_policy_status_changed_for_unset_transitions() {
    let mut base = config_with(entry("allow-1"));
    base.status = Some("advisory".to_string());
    let mut head = base.clone();
    head.status = None;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::PolicyStatusChanged
            && change.severity == PolicyChangeSeverity::Review
            && change.allow_id == "policy.status"
            && change.message.contains("advisory -> <unset>")
    }));
}

#[test]
fn detects_required_metadata_added_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.owner.clear();
    base_entry.reason.clear();
    base_entry.classification.clear();
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OwnerAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReasonAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ClassificationAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}
