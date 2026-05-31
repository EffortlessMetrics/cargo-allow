use super::*;

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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyStatusWeakened)
        .unwrap_or_else(|| std::panic::panic_any("policy status weakening should be reported"));
    assert_eq!(
        change
            .policy_status
            .as_ref()
            .map(|status| (status.before.as_deref(), status.after.as_deref())),
        Some((Some("active"), Some("advisory")))
    );
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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyOwnerRemoved)
        .unwrap_or_else(|| std::panic::panic_any("policy owner removal should be reported"));
    assert_eq!(
        change.metadata.as_ref().map(|metadata| (
            metadata.field,
            metadata.before.as_deref(),
            metadata.after.as_deref()
        )),
        Some((MetadataChangeField::Owner, Some("core/policy"), None))
    );
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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyOwnerUnassigned)
        .unwrap_or_else(|| std::panic::panic_any("policy owner unassignment should be reported"));
    assert_eq!(
        change.metadata.as_ref().map(|metadata| (
            metadata.field,
            metadata.before.as_deref(),
            metadata.after.as_deref()
        )),
        Some((
            MetadataChangeField::Owner,
            Some("core/policy"),
            Some("unowned")
        ))
    );
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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyOwnerAdded)
        .unwrap_or_else(|| std::panic::panic_any("policy owner addition should be reported"));
    assert_eq!(
        change.metadata.as_ref().map(|metadata| (
            metadata.field,
            metadata.before.as_deref(),
            metadata.after.as_deref()
        )),
        Some((MetadataChangeField::Owner, None, Some("core/policy")))
    );
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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyOwnerChanged)
        .unwrap_or_else(|| std::panic::panic_any("policy owner change should be reported"));
    assert_eq!(
        change.metadata.as_ref().map(|metadata| (
            metadata.field,
            metadata.before.as_deref(),
            metadata.after.as_deref()
        )),
        Some((
            MetadataChangeField::Owner,
            Some("core/policy"),
            Some("security")
        ))
    );
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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyStatusTightened)
        .unwrap_or_else(|| std::panic::panic_any("policy status tightening should be reported"));
    assert_eq!(
        change
            .policy_status
            .as_ref()
            .map(|status| (status.before.as_deref(), status.after.as_deref())),
        Some((Some("advisory"), Some("active")))
    );
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
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::PolicyStatusChanged)
        .unwrap_or_else(|| std::panic::panic_any("policy status change should be reported"));
    assert_eq!(
        change
            .policy_status
            .as_ref()
            .map(|status| (status.before.as_deref(), status.after.as_deref())),
        Some((Some("advisory"), None))
    );
}
