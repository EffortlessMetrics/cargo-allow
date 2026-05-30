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
    BaselineDebtNormalized,
    KindChanged,
    FamilyChanged,
    ScopeBroadened,
    ScopeNarrowed,
    SelectorPrecisionDecreased,
    SelectorPrecisionIncreased,
    CreatedAdded,
    CreatedChanged,
    CreatedRemoved,
    ExpiryExtended,
    ExpiryShortened,
    ReviewAfterExtended,
    ReviewAfterShortened,
    EvidenceAdded,
    EvidenceRemoved,
    LinkAdded,
    LinkRemoved,
    OwnerAdded,
    OwnerChanged,
    OwnerRemoved,
    ReasonAdded,
    ReasonChanged,
    ReasonRemoved,
    ClassificationAdded,
    ClassificationChanged,
    ClassificationRemoved,
    OccurrenceLimitTightened,
    OccurrenceLimitLoosened,
}

impl PolicyChangeKind {
    pub const ALL: &[Self] = &[
        Self::AddedAllow,
        Self::RemovedAllow,
        Self::BaselineDebtAdded,
        Self::BaselineDebtNormalized,
        Self::KindChanged,
        Self::FamilyChanged,
        Self::ScopeBroadened,
        Self::ScopeNarrowed,
        Self::SelectorPrecisionDecreased,
        Self::SelectorPrecisionIncreased,
        Self::CreatedAdded,
        Self::CreatedChanged,
        Self::CreatedRemoved,
        Self::ExpiryExtended,
        Self::ExpiryShortened,
        Self::ReviewAfterExtended,
        Self::ReviewAfterShortened,
        Self::EvidenceAdded,
        Self::EvidenceRemoved,
        Self::LinkAdded,
        Self::LinkRemoved,
        Self::OwnerAdded,
        Self::OwnerChanged,
        Self::OwnerRemoved,
        Self::ReasonAdded,
        Self::ReasonChanged,
        Self::ReasonRemoved,
        Self::ClassificationAdded,
        Self::ClassificationChanged,
        Self::ClassificationRemoved,
        Self::OccurrenceLimitTightened,
        Self::OccurrenceLimitLoosened,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AddedAllow => "added_allow",
            Self::RemovedAllow => "removed_allow",
            Self::BaselineDebtAdded => "baseline_debt_added",
            Self::BaselineDebtNormalized => "baseline_debt_normalized",
            Self::KindChanged => "kind_changed",
            Self::FamilyChanged => "family_changed",
            Self::ScopeBroadened => "scope_broadened",
            Self::ScopeNarrowed => "scope_narrowed",
            Self::SelectorPrecisionDecreased => "selector_precision_decreased",
            Self::SelectorPrecisionIncreased => "selector_precision_increased",
            Self::CreatedAdded => "created_added",
            Self::CreatedChanged => "created_changed",
            Self::CreatedRemoved => "created_removed",
            Self::ExpiryExtended => "expiry_extended",
            Self::ExpiryShortened => "expiry_shortened",
            Self::ReviewAfterExtended => "review_after_extended",
            Self::ReviewAfterShortened => "review_after_shortened",
            Self::EvidenceAdded => "evidence_added",
            Self::EvidenceRemoved => "evidence_removed",
            Self::LinkAdded => "link_added",
            Self::LinkRemoved => "link_removed",
            Self::OwnerAdded => "owner_added",
            Self::OwnerChanged => "owner_changed",
            Self::OwnerRemoved => "owner_removed",
            Self::ReasonAdded => "reason_added",
            Self::ReasonChanged => "reason_changed",
            Self::ReasonRemoved => "reason_removed",
            Self::ClassificationAdded => "classification_added",
            Self::ClassificationChanged => "classification_changed",
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
    pub const ALL: &[Self] = &[Self::Improvement, Self::Review, Self::Fail];

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
