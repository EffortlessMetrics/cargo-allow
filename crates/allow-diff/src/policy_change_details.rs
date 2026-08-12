#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorPrecisionChange {
    pub before: u32,
    pub after: u32,
    pub removed_fields: Vec<&'static str>,
    pub added_fields: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionIdentityChange {
    pub field: ExceptionIdentityChangeField,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorIdentityChange {
    pub changed_fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionIdentityChangeField {
    Kind,
    Family,
}

impl ExceptionIdentityChangeField {
    pub const ALL: &[Self] = &[Self::Kind, Self::Family];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kind => "kind",
            Self::Family => "family",
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataChange {
    pub field: MetadataChangeField,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataChangeField {
    Owner,
    Reason,
    Classification,
}

impl MetadataChangeField {
    pub const ALL: &[Self] = &[Self::Owner, Self::Reason, Self::Classification];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Reason => "reason",
            Self::Classification => "classification",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementChange {
    pub field: RequirementChangeField,
    pub before: bool,
    pub after: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementChangeField {
    OwnerRequired,
    ReasonRequired,
    ClassificationRequired,
    EvidenceRequired,
    ExpiresOrReviewAfterRequired,
    AllowBareAllowAttributes,
    LintPolicyIdRequired,
    StaleEntriesFail,
    UnsafeEvidenceRequired,
    UnsafeVerifiedEvidenceRequired,
    UnsafeSafetyCommentRequired,
}

impl RequirementChangeField {
    pub const ALL: &[Self] = &[
        Self::OwnerRequired,
        Self::ReasonRequired,
        Self::ClassificationRequired,
        Self::EvidenceRequired,
        Self::ExpiresOrReviewAfterRequired,
        Self::AllowBareAllowAttributes,
        Self::LintPolicyIdRequired,
        Self::StaleEntriesFail,
        Self::UnsafeEvidenceRequired,
        Self::UnsafeVerifiedEvidenceRequired,
        Self::UnsafeSafetyCommentRequired,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnerRequired => "owner_required",
            Self::ReasonRequired => "reason_required",
            Self::ClassificationRequired => "classification_required",
            Self::EvidenceRequired => "evidence_required",
            Self::ExpiresOrReviewAfterRequired => "expires_or_review_after_required",
            Self::AllowBareAllowAttributes => "allow_bare_allow_attributes",
            Self::LintPolicyIdRequired => "lint_policy_id_required",
            Self::StaleEntriesFail => "stale_entries_fail",
            Self::UnsafeEvidenceRequired => "unsafe.evidence_required",
            Self::UnsafeVerifiedEvidenceRequired => "unsafe.verified_evidence_required",
            Self::UnsafeSafetyCommentRequired => "unsafe.safety_comment_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyStatusChange {
    pub before: Option<String>,
    pub after: Option<String>,
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
