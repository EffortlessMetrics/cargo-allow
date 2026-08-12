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
    let change = changes
        .iter()
        .find(|change| change.allow_id == "requirements.owner_required")
        .unwrap_or_else(|| std::panic::panic_any("owner requirement change should be reported"));
    assert_eq!(
        change.requirement.as_ref().map(|requirement| (
            requirement.field,
            requirement.before,
            requirement.after
        )),
        Some((RequirementChangeField::OwnerRequired, true, false))
    );
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
    let change = changes
        .iter()
        .find(|change| change.allow_id == "requirements.evidence_required")
        .unwrap_or_else(|| std::panic::panic_any("evidence requirement change should be reported"));
    assert_eq!(
        change.requirement.as_ref().map(|requirement| (
            requirement.field,
            requirement.before,
            requirement.after
        )),
        Some((RequirementChangeField::EvidenceRequired, false, true))
    );
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

#[test]
fn detects_unsafe_verified_evidence_requirement_polarity() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.requirements.unsafe_verified_evidence_required = true;

    let tightened = policy_changes(&base, &head);
    assert!(tightened.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementTightened
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "requirements.unsafe.verified_evidence_required"
            && change.message.contains("false -> true")
            && change.requirement.as_ref().is_some_and(|requirement| {
                requirement.field == RequirementChangeField::UnsafeVerifiedEvidenceRequired
                    && !requirement.before
                    && requirement.after
            })
    }));

    let loosened = policy_changes(&head, &base);
    assert!(loosened.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementLoosened
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "requirements.unsafe.verified_evidence_required"
            && change.message.contains("true -> false")
    }));
}
