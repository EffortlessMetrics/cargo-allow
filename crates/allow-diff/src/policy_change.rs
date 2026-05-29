#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChange {
    pub allow_id: String,
    pub kind: PolicyChangeKind,
    pub severity: PolicyChangeSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChangeKind {
    AddedAllow,
    RemovedAllow,
    BaselineDebtAdded,
    KindChanged,
    FamilyChanged,
    ScopeBroadened,
    ScopeNarrowed,
    SelectorPrecisionDecreased,
    SelectorPrecisionIncreased,
    ExpiryExtended,
    ExpiryShortened,
    ReviewAfterExtended,
    ReviewAfterShortened,
    EvidenceAdded,
    EvidenceRemoved,
    OwnerAdded,
    OwnerRemoved,
    ReasonAdded,
    ReasonRemoved,
    ClassificationAdded,
    ClassificationRemoved,
    OccurrenceLimitTightened,
    OccurrenceLimitLoosened,
}

impl PolicyChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AddedAllow => "added_allow",
            Self::RemovedAllow => "removed_allow",
            Self::BaselineDebtAdded => "baseline_debt_added",
            Self::KindChanged => "kind_changed",
            Self::FamilyChanged => "family_changed",
            Self::ScopeBroadened => "scope_broadened",
            Self::ScopeNarrowed => "scope_narrowed",
            Self::SelectorPrecisionDecreased => "selector_precision_decreased",
            Self::SelectorPrecisionIncreased => "selector_precision_increased",
            Self::ExpiryExtended => "expiry_extended",
            Self::ExpiryShortened => "expiry_shortened",
            Self::ReviewAfterExtended => "review_after_extended",
            Self::ReviewAfterShortened => "review_after_shortened",
            Self::EvidenceAdded => "evidence_added",
            Self::EvidenceRemoved => "evidence_removed",
            Self::OwnerAdded => "owner_added",
            Self::OwnerRemoved => "owner_removed",
            Self::ReasonAdded => "reason_added",
            Self::ReasonRemoved => "reason_removed",
            Self::ClassificationAdded => "classification_added",
            Self::ClassificationRemoved => "classification_removed",
            Self::OccurrenceLimitTightened => "occurrence_limit_tightened",
            Self::OccurrenceLimitLoosened => "occurrence_limit_loosened",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChangeSeverity {
    Improvement,
    Review,
    Fail,
}

impl PolicyChangeSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Improvement => "improvement",
            Self::Review => "review",
            Self::Fail => "fail",
        }
    }

    pub fn fails(self) -> bool {
        matches!(self, Self::Fail)
    }
}
