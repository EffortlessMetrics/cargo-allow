use super::*;

#[test]
fn detects_evidence_removed_and_lifecycle_extended() {
    let base = config_with(entry("allow-1"));
    let mut weaker = entry("allow-1");
    weaker.evidence.clear();
    weaker.lifecycle.expires = Some("2026-12-01".to_string());
    weaker.lifecycle.review_after = Some("2026-10-01".to_string());
    let head = config_with(weaker);

    let changes = policy_changes(&base, &head);

    let evidence_removed = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceRemoved)
        .unwrap_or_else(|| std::panic::panic_any("evidence removal should be reported"));
    let evidence = evidence_removed
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence removal should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Evidence);
    assert_eq!(
        evidence.removed,
        vec!["test:range_is_validated".to_string()]
    );
    assert!(evidence.added.is_empty());
    let expiry = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::ExpiryExtended)
        .unwrap_or_else(|| std::panic::panic_any("expiry extension should be reported"));
    assert_eq!(expiry.severity, PolicyChangeSeverity::Review);
    assert_eq!(
        expiry.lifecycle.as_ref().map(|change| (
            change.field,
            change.before.as_deref(),
            change.after.as_deref()
        )),
        Some((
            LifecycleChangeField::Expires,
            Some("2026-09-01"),
            Some("2026-12-01")
        ))
    );
    assert!(changes.iter().any(
        |change| change.kind == PolicyChangeKind::ReviewAfterExtended
            && change.severity == PolicyChangeSeverity::Review
    ));
}

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
    assert_eq!(
        created_removed.lifecycle.as_ref().map(|change| (
            change.field,
            change.before.as_deref(),
            change.after.as_deref()
        )),
        Some((LifecycleChangeField::Created, Some("2026-05-26"), None))
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
fn detects_evidence_added_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Improvement);
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Evidence);
    assert!(evidence.removed.is_empty());
    assert_eq!(evidence.added, vec!["test:range_is_validated".to_string()]);
}

#[test]
fn detects_traceability_link_changes() {
    let mut base_entry = entry("allow-1");
    base_entry.links = vec!["adr:docs/adr/0001.md".to_string(), "issue:123".to_string()];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.links = vec!["issue:123".to_string(), "pr:456".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let link_removed = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::LinkRemoved)
        .unwrap_or_else(|| std::panic::panic_any("link removal should be reported"));
    assert_eq!(link_removed.severity, PolicyChangeSeverity::Review);
    assert!(link_removed.message.contains("traceability link removed"));
    let evidence = link_removed
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("link removal should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Links);
    assert_eq!(evidence.removed, vec!["adr:docs/adr/0001.md".to_string()]);
    assert!(evidence.added.is_empty());
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::LinkAdded
            && change.severity == PolicyChangeSeverity::Improvement
            && change.message.contains("traceability link added")
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
