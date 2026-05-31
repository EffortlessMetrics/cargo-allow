pub use crate::policy_change_details::{
    EvidenceChange, EvidenceChangeField, ExceptionIdentityChange, ExceptionIdentityChangeField,
    LifecycleChange, LifecycleChangeField, MetadataChange, MetadataChangeField,
    OccurrenceLimitChange, PolicyStatusChange, RequirementChange, RequirementChangeField,
    ScopeChange, ScopeChangeField, SelectorIdentityChange, SelectorPrecisionChange,
};
pub use crate::policy_change_kind::{PolicyChangeKind, PolicyChangeSeverity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChange {
    pub allow_id: String,
    pub kind: PolicyChangeKind,
    pub severity: PolicyChangeSeverity,
    pub message: String,
    pub exception_identity: Option<ExceptionIdentityChange>,
    pub selector_identity: Option<SelectorIdentityChange>,
    pub selector_precision: Option<SelectorPrecisionChange>,
    pub scope: Option<ScopeChange>,
    pub occurrence_limit: Option<OccurrenceLimitChange>,
    pub lifecycle: Option<LifecycleChange>,
    pub evidence: Option<EvidenceChange>,
    pub metadata: Option<MetadataChange>,
    pub requirement: Option<RequirementChange>,
    pub policy_status: Option<PolicyStatusChange>,
}

impl PolicyChange {
    pub fn new(
        allow_id: impl Into<String>,
        kind: PolicyChangeKind,
        severity: PolicyChangeSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            allow_id: allow_id.into(),
            kind,
            severity,
            message: message.into(),
            exception_identity: None,
            selector_identity: None,
            selector_precision: None,
            scope: None,
            occurrence_limit: None,
            lifecycle: None,
            evidence: None,
            metadata: None,
            requirement: None,
            policy_status: None,
        }
    }

    pub fn with_selector_precision(mut self, selector_precision: SelectorPrecisionChange) -> Self {
        self.selector_precision = Some(selector_precision);
        self
    }

    pub fn with_exception_identity(mut self, exception_identity: ExceptionIdentityChange) -> Self {
        self.exception_identity = Some(exception_identity);
        self
    }

    pub fn with_selector_identity(mut self, selector_identity: SelectorIdentityChange) -> Self {
        self.selector_identity = Some(selector_identity);
        self
    }

    pub fn with_scope(mut self, scope: ScopeChange) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn with_occurrence_limit(mut self, occurrence_limit: OccurrenceLimitChange) -> Self {
        self.occurrence_limit = Some(occurrence_limit);
        self
    }

    pub fn with_lifecycle(mut self, lifecycle: LifecycleChange) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    pub fn with_evidence(mut self, evidence: EvidenceChange) -> Self {
        self.evidence = Some(evidence);
        self
    }

    pub fn with_metadata(mut self, metadata: MetadataChange) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_requirement(mut self, requirement: RequirementChange) -> Self {
        self.requirement = Some(requirement);
        self
    }

    pub fn with_policy_status(mut self, policy_status: PolicyStatusChange) -> Self {
        self.policy_status = Some(policy_status);
        self
    }
}
