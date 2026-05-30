use super::*;

#[test]
fn lint_policy_reference_must_match_entry_id() {
    let finding = lint_finding_with_policy("allow-other");
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(lint_entry("allow-lint"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::InvalidSelector
            && outcome.message.contains("policy:allow-other")
    }));
}

#[test]
fn lint_policy_reference_matching_entry_id_passes() {
    let finding = lint_finding_with_policy("allow-lint");
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(lint_entry("allow-lint"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn bare_allow_attribute_fails_when_policy_disallows_it() {
    let finding = lint_finding("allow_attribute");
    let mut cfg = AllowConfig::empty();
    cfg.requirements.allow_bare_allow_attributes = false;
    cfg.allow
        .push(lint_entry_with_family("allow-lint", "allow_attribute"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::InvalidSelector
            && outcome
                .message
                .contains("allow_bare_allow_attributes=false")
    }));
}

#[test]
fn bare_allow_attribute_passes_when_policy_allows_it() {
    let finding = lint_finding("allow_attribute");
    let mut cfg = AllowConfig::empty();
    cfg.requirements.allow_bare_allow_attributes = true;
    cfg.allow
        .push(lint_entry_with_family("allow-lint", "allow_attribute"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn lint_policy_id_is_required_when_configured() {
    let finding = lint_finding("expect_attribute");
    let mut cfg = AllowConfig::empty();
    cfg.requirements.lint_policy_id_required = true;
    cfg.allow.push(lint_entry("allow-lint"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::InvalidSelector
            && outcome
                .message
                .contains("without required policy:<allow-id> reference")
    }));
}

#[test]
fn lint_policy_id_is_optional_by_default() {
    let finding = lint_finding("expect_attribute");
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(lint_entry("allow-lint"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}
