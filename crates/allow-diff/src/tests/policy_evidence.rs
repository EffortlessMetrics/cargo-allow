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
fn detects_local_evidence_removed_with_specific_message() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence = vec!["doc:docs/safety/parser-spans.md".to_string()];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence.clear();
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceRemoved)
        .unwrap_or_else(|| std::panic::panic_any("local evidence removal should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert!(change.message.contains("local evidence removed"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence removal should include values"));
    assert_eq!(
        evidence.removed,
        vec!["doc:docs/safety/parser-spans.md".to_string()]
    );
}

#[test]
fn detects_weak_evidence_removed_as_improvement_when_typed_evidence_remains() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence = vec![
        "legacy-policy:proc-cargo-install-cargo-deny".to_string(),
        "binary:cargo".to_string(),
    ];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence = vec!["legacy-policy:proc-cargo-install-cargo-deny".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceRemoved)
        .unwrap_or_else(|| std::panic::panic_any("weak evidence removal should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Improvement);
    assert!(change.message.contains("weak evidence removed"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence removal should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Evidence);
    assert_eq!(evidence.removed, vec!["binary:cargo".to_string()]);
    assert!(evidence.added.is_empty());
}

#[test]
fn detects_weak_evidence_removed_without_typed_replacement_as_review_required() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence = vec!["binary:cargo".to_string()];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence.clear();
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceRemoved)
        .unwrap_or_else(|| std::panic::panic_any("weak evidence removal should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Review);
    assert!(change.message.contains("weak evidence removed"));
}

#[test]
fn detects_local_evidence_added_as_improvement_when_source_tree_relative() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence = vec!["doc:docs/safety/parser-spans.md".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| std::panic::panic_any("local evidence addition should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Improvement);
    assert!(change.message.contains("evidence added"));
}

#[test]
fn detects_weak_evidence_added_as_review_required() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence = vec![
        "spreadsheet:manual-review".to_string(),
        "test:".to_string(),
        "untyped review note".to_string(),
    ];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| std::panic::panic_any("weak evidence addition should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Review);
    assert!(change.message.contains("weak evidence added"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("weak evidence addition should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Evidence);
    assert!(evidence.removed.is_empty());
    assert_eq!(
        evidence.added,
        vec![
            "spreadsheet:manual-review".to_string(),
            "test:".to_string(),
            "untyped review note".to_string()
        ]
    );
}

#[test]
fn detects_invalid_local_evidence_added_as_failure() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence = vec!["doc:../outside.md".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| {
            std::panic::panic_any("invalid local evidence addition should be reported")
        });
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert!(change.message.contains("invalid local evidence added"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Evidence);
    assert!(evidence.removed.is_empty());
    assert_eq!(evidence.added, vec!["doc:../outside.md".to_string()]);
}

#[test]
fn detects_redundant_current_dir_local_evidence_added_as_failure() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.evidence = vec!["doc:docs/./safety.md".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| {
            std::panic::panic_any("redundant segment evidence addition should be reported")
        });
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert!(change.message.contains("invalid local evidence added"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Evidence);
    assert!(evidence.removed.is_empty());
    assert_eq!(evidence.added, vec!["doc:docs/./safety.md".to_string()]);
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
    assert_eq!(link_removed.severity, PolicyChangeSeverity::Fail);
    assert!(
        link_removed
            .message
            .contains("local traceability link removed")
    );
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
fn detects_non_local_traceability_link_removed_as_review_required() {
    let mut base_entry = entry("allow-1");
    base_entry.links = vec!["issue:123".to_string(), "pr:456".to_string()];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.links = vec!["pr:456".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let link_removed = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::LinkRemoved)
        .unwrap_or_else(|| std::panic::panic_any("link removal should be reported"));
    assert_eq!(link_removed.severity, PolicyChangeSeverity::Review);
    assert!(link_removed.message.contains("traceability link removed"));
    assert!(
        !link_removed
            .message
            .contains("local traceability link removed")
    );
    let evidence = link_removed
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("link removal should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Links);
    assert_eq!(evidence.removed, vec!["issue:123".to_string()]);
    assert!(evidence.added.is_empty());
}

#[test]
fn detects_weak_traceability_link_removed_as_improvement_when_typed_link_remains() {
    let mut base_entry = entry("allow-1");
    base_entry.links = vec![
        "issue:123".to_string(),
        "spreadsheet:manual-review".to_string(),
    ];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.links = vec!["issue:123".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::LinkRemoved)
        .unwrap_or_else(|| std::panic::panic_any("weak link removal should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Improvement);
    assert!(change.message.contains("weak traceability link removed"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("link removal should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Links);
    assert_eq!(
        evidence.removed,
        vec!["spreadsheet:manual-review".to_string()]
    );
    assert!(evidence.added.is_empty());
}

#[test]
fn detects_weak_traceability_link_removed_without_typed_replacement_as_review_required() {
    let mut base_entry = entry("allow-1");
    base_entry.links = vec!["spreadsheet:manual-review".to_string()];
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.links.clear();
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::LinkRemoved)
        .unwrap_or_else(|| std::panic::panic_any("weak link removal should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Review);
    assert!(change.message.contains("traceability link removed"));
}

#[test]
fn detects_weak_traceability_link_added_as_review_required() {
    let mut base_entry = entry("allow-1");
    base_entry.links = Vec::new();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.links = vec![
        "manual review note".to_string(),
        "spreadsheet:manual-review".to_string(),
    ];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::LinkAdded)
        .unwrap_or_else(|| std::panic::panic_any("weak link addition should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Review);
    assert!(change.message.contains("weak traceability link added"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("link addition should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Links);
    assert!(evidence.removed.is_empty());
    assert_eq!(
        evidence.added,
        vec![
            "manual review note".to_string(),
            "spreadsheet:manual-review".to_string()
        ]
    );
}

#[test]
fn detects_invalid_local_traceability_link_added_as_failure() {
    let mut base_entry = entry("allow-1");
    base_entry.links = Vec::new();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.links = vec!["doc:../outside.md".to_string()];
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::LinkAdded)
        .unwrap_or_else(|| std::panic::panic_any("invalid link addition should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert!(change.message.contains("invalid traceability link added"));
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("link addition should include values"));
    assert_eq!(evidence.field, EvidenceChangeField::Links);
    assert!(evidence.removed.is_empty());
    assert_eq!(evidence.added, vec!["doc:../outside.md".to_string()]);
}
