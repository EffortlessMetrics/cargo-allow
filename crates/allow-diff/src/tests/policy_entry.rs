use super::*;

#[test]
fn detects_occurrence_limit_tightened_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.occurrence_limit = Some(4);
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.occurrence_limit = Some(2);
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OccurrenceLimitTightened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_new_occurrence_limit_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.occurrence_limit = None;
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OccurrenceLimitTightened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_added_baseline_debt_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut added = entry("allow-2");
    added.classification = "baseline_debt".to_string();
    let mut head = base.clone();
    head.allow.push(added);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::BaselineDebtAdded
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_baseline_debt_normalized_as_failure() {
    let mut base_entry = entry("allow-1");
    base_entry.classification = "baseline_debt".to_string();
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::BaselineDebtNormalized
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("baseline_debt")
    }));
}

#[test]
fn detects_baseline_debt_introduced_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head_entry = entry("allow-1");
    head_entry.classification = "baseline_debt".to_string();
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::BaselineDebtIntroduced
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("baseline_debt")
    }));
    assert!(
        !changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ClassificationChanged),
        "baseline debt introduction should not be downgraded to a generic classification change"
    );
}

#[test]
fn detects_removed_allow_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.allow.push(entry("allow-2"));
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.allow_id == "allow-2"
            && change.kind == PolicyChangeKind::RemovedAllow
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_occurrence_limit_increase_as_loosened() {
    let mut base_entry = entry("allow-1");
    base_entry.occurrence_limit = Some(1);
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.occurrence_limit = Some(3);
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OccurrenceLimitLoosened
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_non_baseline_added_allow_for_review() {
    let base = AllowConfig::empty();
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, PolicyChangeKind::AddedAllow);
    assert_eq!(changes[0].severity, PolicyChangeSeverity::Review);
}
