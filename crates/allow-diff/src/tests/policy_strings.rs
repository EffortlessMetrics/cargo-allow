use super::*;

#[test]
fn policy_change_string_helpers_cover_all_public_variants() {
    let cases = [
        (PolicyChangeKind::AddedAllow, "added_allow"),
        (PolicyChangeKind::RemovedAllow, "removed_allow"),
        (PolicyChangeKind::BaselineDebtAdded, "baseline_debt_added"),
        (
            PolicyChangeKind::BaselineDebtIntroduced,
            "baseline_debt_introduced",
        ),
        (
            PolicyChangeKind::BaselineDebtNormalized,
            "baseline_debt_normalized",
        ),
        (PolicyChangeKind::KindChanged, "kind_changed"),
        (PolicyChangeKind::FamilyChanged, "family_changed"),
        (PolicyChangeKind::FamilyRuleAdded, "family_rule_added"),
        (PolicyChangeKind::FamilyRuleRemoved, "family_rule_removed"),
        (
            PolicyChangeKind::AmbiguousClassification,
            "ambiguous_classification",
        ),
        (PolicyChangeKind::ScopeBroadened, "scope_broadened"),
        (PolicyChangeKind::ScopeChanged, "scope_changed"),
        (PolicyChangeKind::ScopeNarrowed, "scope_narrowed"),
        (PolicyChangeKind::SelectorChanged, "selector_changed"),
        (
            PolicyChangeKind::SelectorPrecisionDecreased,
            "selector_precision_decreased",
        ),
        (
            PolicyChangeKind::SelectorPrecisionIncreased,
            "selector_precision_increased",
        ),
        (PolicyChangeKind::CreatedAdded, "created_added"),
        (PolicyChangeKind::CreatedChanged, "created_changed"),
        (PolicyChangeKind::CreatedRemoved, "created_removed"),
        (PolicyChangeKind::ExpiryExtended, "expiry_extended"),
        (PolicyChangeKind::ExpiryShortened, "expiry_shortened"),
        (
            PolicyChangeKind::ReviewAfterExtended,
            "review_after_extended",
        ),
        (
            PolicyChangeKind::ReviewAfterShortened,
            "review_after_shortened",
        ),
        (PolicyChangeKind::EvidenceAdded, "evidence_added"),
        (PolicyChangeKind::EvidenceRemoved, "evidence_removed"),
        (PolicyChangeKind::LinkAdded, "link_added"),
        (PolicyChangeKind::LinkRemoved, "link_removed"),
        (PolicyChangeKind::OwnerAdded, "owner_added"),
        (PolicyChangeKind::OwnerChanged, "owner_changed"),
        (PolicyChangeKind::OwnerRemoved, "owner_removed"),
        (PolicyChangeKind::OwnerUnassigned, "owner_unassigned"),
        (PolicyChangeKind::PolicyOwnerAdded, "policy_owner_added"),
        (PolicyChangeKind::PolicyOwnerChanged, "policy_owner_changed"),
        (PolicyChangeKind::PolicyOwnerRemoved, "policy_owner_removed"),
        (
            PolicyChangeKind::PolicyOwnerUnassigned,
            "policy_owner_unassigned",
        ),
        (
            PolicyChangeKind::PolicyStatusChanged,
            "policy_status_changed",
        ),
        (
            PolicyChangeKind::PolicyStatusWeakened,
            "policy_status_weakened",
        ),
        (
            PolicyChangeKind::PolicyStatusTightened,
            "policy_status_tightened",
        ),
        (PolicyChangeKind::ReasonAdded, "reason_added"),
        (PolicyChangeKind::ReasonChanged, "reason_changed"),
        (PolicyChangeKind::ReasonRemoved, "reason_removed"),
        (
            PolicyChangeKind::RequirementLoosened,
            "requirement_loosened",
        ),
        (
            PolicyChangeKind::RequirementTightened,
            "requirement_tightened",
        ),
        (
            PolicyChangeKind::WorkspaceIgnoredAdded,
            "workspace_ignored_added",
        ),
        (
            PolicyChangeKind::WorkspaceIgnoredRemoved,
            "workspace_ignored_removed",
        ),
        (
            PolicyChangeKind::WorkspaceGeneratedAdded,
            "workspace_generated_added",
        ),
        (
            PolicyChangeKind::WorkspaceGeneratedRemoved,
            "workspace_generated_removed",
        ),
        (
            PolicyChangeKind::ClassificationAdded,
            "classification_added",
        ),
        (
            PolicyChangeKind::ClassificationChanged,
            "classification_changed",
        ),
        (
            PolicyChangeKind::ClassificationRemoved,
            "classification_removed",
        ),
        (
            PolicyChangeKind::OccurrenceLimitTightened,
            "occurrence_limit_tightened",
        ),
        (
            PolicyChangeKind::OccurrenceLimitLoosened,
            "occurrence_limit_loosened",
        ),
    ];

    for (kind, expected) in cases {
        assert_eq!(kind.as_str(), expected);
    }
    assert_eq!(PolicyChangeSeverity::Improvement.as_str(), "improvement");
    assert_eq!(PolicyChangeSeverity::Review.as_str(), "review");
    assert_eq!(PolicyChangeSeverity::Fail.as_str(), "fail");
    assert!(!PolicyChangeSeverity::Review.fails());
    assert!(PolicyChangeSeverity::Fail.fails());
}
