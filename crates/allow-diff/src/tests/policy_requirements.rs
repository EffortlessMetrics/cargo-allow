use super::*;

#[test]
fn detects_requirement_loosened_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.requirements.owner_required = false;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementLoosened
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "requirements.owner_required"
            && change.message.contains("true -> false")
    }));
}

#[test]
fn detects_requirement_tightened_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.requirements.evidence_required = false;
    let mut head = base.clone();
    head.requirements.evidence_required = true;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementTightened
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "requirements.evidence_required"
            && change.message.contains("false -> true")
    }));
}

#[test]
fn detects_allow_bare_allow_attributes_polarity() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.requirements.allow_bare_allow_attributes = true;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementLoosened
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "requirements.allow_bare_allow_attributes"
            && change.message.contains("false -> true")
    }));

    let changes = policy_changes(&head, &base);
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementTightened
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "requirements.allow_bare_allow_attributes"
            && change.message.contains("true -> false")
    }));
}
