#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChange {
    pub allow_id: String,
    pub kind: PolicyChangeKind,
    pub severity: PolicyChangeSeverity,
    pub message: String,
    pub selector_precision: Option<SelectorPrecisionChange>,
    pub scope: Option<ScopeChange>,
    pub occurrence_limit: Option<OccurrenceLimitChange>,
    pub lifecycle: Option<LifecycleChange>,
    pub evidence: Option<EvidenceChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorPrecisionChange {
    pub before: u32,
    pub after: u32,
    pub removed_fields: Vec<&'static str>,
    pub added_fields: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeChange {
    pub field: ScopeChangeField,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceLimitChange {
    pub before: Option<u32>,
    pub after: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleChange {
    pub field: LifecycleChangeField,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceChange {
    pub field: EvidenceChangeField,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceChangeField {
    Evidence,
    Links,
}

impl EvidenceChangeField {
    pub const ALL: &[Self] = &[Self::Evidence, Self::Links];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Links => "links",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleChangeField {
    Created,
    Expires,
    ReviewAfter,
}

impl LifecycleChangeField {
    pub const ALL: &[Self] = &[Self::Created, Self::Expires, Self::ReviewAfter];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Expires => "expires",
            Self::ReviewAfter => "review_after",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeChangeField {
    Path,
    Glob,
    SelectorGlob,
    Effective,
}

impl ScopeChangeField {
    pub const ALL: &[Self] = &[Self::Path, Self::Glob, Self::SelectorGlob, Self::Effective];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Glob => "glob",
            Self::SelectorGlob => "selector.glob",
            Self::Effective => "effective",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChangeKind {
    AddedAllow,
    RemovedAllow,
    BaselineDebtAdded,
    BaselineDebtIntroduced,
    BaselineDebtNormalized,
    KindChanged,
    FamilyChanged,
    ScopeBroadened,
    ScopeChanged,
    ScopeNarrowed,
    SelectorChanged,
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
    OwnerUnassigned,
    PolicyOwnerAdded,
    PolicyOwnerChanged,
    PolicyOwnerRemoved,
    PolicyOwnerUnassigned,
    PolicyStatusChanged,
    PolicyStatusWeakened,
    PolicyStatusTightened,
    ReasonAdded,
    ReasonChanged,
    ReasonRemoved,
    RequirementLoosened,
    RequirementTightened,
    WorkspaceIgnoredAdded,
    WorkspaceIgnoredRemoved,
    WorkspaceGeneratedAdded,
    WorkspaceGeneratedRemoved,
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
        Self::BaselineDebtIntroduced,
        Self::BaselineDebtNormalized,
        Self::KindChanged,
        Self::FamilyChanged,
        Self::ScopeBroadened,
        Self::ScopeChanged,
        Self::ScopeNarrowed,
        Self::SelectorChanged,
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
        Self::OwnerUnassigned,
        Self::PolicyOwnerAdded,
        Self::PolicyOwnerChanged,
        Self::PolicyOwnerRemoved,
        Self::PolicyOwnerUnassigned,
        Self::PolicyStatusChanged,
        Self::PolicyStatusWeakened,
        Self::PolicyStatusTightened,
        Self::ReasonAdded,
        Self::ReasonChanged,
        Self::ReasonRemoved,
        Self::RequirementLoosened,
        Self::RequirementTightened,
        Self::WorkspaceIgnoredAdded,
        Self::WorkspaceIgnoredRemoved,
        Self::WorkspaceGeneratedAdded,
        Self::WorkspaceGeneratedRemoved,
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
            Self::BaselineDebtIntroduced => "baseline_debt_introduced",
            Self::BaselineDebtNormalized => "baseline_debt_normalized",
            Self::KindChanged => "kind_changed",
            Self::FamilyChanged => "family_changed",
            Self::ScopeBroadened => "scope_broadened",
            Self::ScopeChanged => "scope_changed",
            Self::ScopeNarrowed => "scope_narrowed",
            Self::SelectorChanged => "selector_changed",
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
            Self::OwnerUnassigned => "owner_unassigned",
            Self::PolicyOwnerAdded => "policy_owner_added",
            Self::PolicyOwnerChanged => "policy_owner_changed",
            Self::PolicyOwnerRemoved => "policy_owner_removed",
            Self::PolicyOwnerUnassigned => "policy_owner_unassigned",
            Self::PolicyStatusChanged => "policy_status_changed",
            Self::PolicyStatusWeakened => "policy_status_weakened",
            Self::PolicyStatusTightened => "policy_status_tightened",
            Self::ReasonAdded => "reason_added",
            Self::ReasonChanged => "reason_changed",
            Self::ReasonRemoved => "reason_removed",
            Self::RequirementLoosened => "requirement_loosened",
            Self::RequirementTightened => "requirement_tightened",
            Self::WorkspaceIgnoredAdded => "workspace_ignored_added",
            Self::WorkspaceIgnoredRemoved => "workspace_ignored_removed",
            Self::WorkspaceGeneratedAdded => "workspace_generated_added",
            Self::WorkspaceGeneratedRemoved => "workspace_generated_removed",
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
