use super::*;

#[test]
fn detects_lifecycle_shortened_as_improvement() {
    let base = config_with(entry("allow-1"));
    let mut tighter = entry("allow-1");
    tighter.lifecycle.expires = Some("2026-08-15".to_string());
    tighter.lifecycle.review_after = Some("2026-07-01".to_string());
    let head = config_with(tighter);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_created_lifecycle_provenance_changes() {
    let base = config_with(entry("allow-1"));
    let mut removed = entry("allow-1");
    removed.lifecycle.created = None;
    let changes = policy_changes(&base, &config_with(removed));

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::CreatedRemoved
            && change.severity == PolicyChangeSeverity::Fail
    }));
    let created_removed = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::CreatedRemoved)
        .unwrap_or_else(|| std::panic::panic_any("created removal should be reported"));
    let expected_created = base
        .allow
        .first()
        .and_then(|entry| entry.lifecycle.created.as_deref());
    assert_eq!(
        created_removed.lifecycle.as_ref().map(|change| (
            change.field,
            change.before.as_deref(),
            change.after.as_deref()
        )),
        Some((LifecycleChangeField::Created, expected_created, None))
    );

    let mut changed = entry("allow-1");
    changed.lifecycle.created = Some("2026-06-01".to_string());
    let changes = policy_changes(&base, &config_with(changed));

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::CreatedChanged
            && change.severity == PolicyChangeSeverity::Review
    }));

    let mut missing = entry("allow-1");
    missing.lifecycle.created = None;
    let changes = policy_changes(&config_with(missing), &config_with(entry("allow-1")));

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::CreatedAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_added_lifecycle_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.lifecycle.expires = None;
    base_entry.lifecycle.review_after = None;
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn lifecycle_never_and_removed_dates_are_classified_by_risk_direction() {
    let mut never_base = entry("allow-1");
    never_base.lifecycle.expires = Some("never".to_string());
    never_base.lifecycle.review_after = Some("never".to_string());
    let base = config_with(never_base);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));

    let base = config_with(entry("allow-1"));
    let mut removed = entry("allow-1");
    removed.lifecycle.expires = None;
    removed.lifecycle.review_after = None;
    let head = config_with(removed);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryExtended
            && change.severity == PolicyChangeSeverity::Review
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterExtended
            && change.severity == PolicyChangeSeverity::Review
    }));
}

#[test]
fn lifecycle_invalid_dates_do_not_create_directional_changes() {
    let mut base_entry = entry("allow-1");
    base_entry.lifecycle.expires = Some("not-a-date".to_string());
    base_entry.lifecycle.review_after = Some("2026-08-01".to_string());
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.lifecycle.expires = Some("2026-12-01".to_string());
    head_entry.lifecycle.review_after = Some("also-not-a-date".to_string());
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(!changes.iter().any(|change| matches!(
        change.kind,
        PolicyChangeKind::ExpiryExtended
            | PolicyChangeKind::ExpiryShortened
            | PolicyChangeKind::ReviewAfterExtended
            | PolicyChangeKind::ReviewAfterShortened
    )));
}
