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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_change() -> PolicyChange {
        PolicyChange::new(
            "allow-test",
            PolicyChangeKind::ScopeChanged,
            PolicyChangeSeverity::Review,
            "policy changed",
        )
    }

    #[test]
    fn new_policy_change_initializes_base_fields_without_details() {
        let change = base_change();

        assert_eq!(change.allow_id, "allow-test");
        assert_eq!(change.kind, PolicyChangeKind::ScopeChanged);
        assert_eq!(change.severity, PolicyChangeSeverity::Review);
        assert_eq!(change.message, "policy changed");
        assert_eq!(change.exception_identity, None);
        assert_eq!(change.selector_identity, None);
        assert_eq!(change.selector_precision, None);
        assert_eq!(change.scope, None);
        assert_eq!(change.occurrence_limit, None);
        assert_eq!(change.lifecycle, None);
        assert_eq!(change.evidence, None);
        assert_eq!(change.metadata, None);
        assert_eq!(change.requirement, None);
        assert_eq!(change.policy_status, None);
    }

    #[test]
    fn policy_change_builders_attach_each_detail_without_changing_base_fields() {
        let selector_precision = SelectorPrecisionChange {
            before: 3,
            after: 2,
            removed_fields: vec!["path"],
            added_fields: vec!["selector.glob"],
        };
        let exception_identity = ExceptionIdentityChange {
            field: ExceptionIdentityChangeField::Kind,
            before: Some("unsafe".to_string()),
            after: Some("panic".to_string()),
        };
        let selector_identity = SelectorIdentityChange {
            changed_fields: vec!["selector.glob", "code"],
        };
        let scope = ScopeChange {
            field: ScopeChangeField::Path,
            before: Some("src/lib.rs".to_string()),
            after: Some("src/main.rs".to_string()),
        };
        let occurrence_limit = OccurrenceLimitChange {
            before: Some(1),
            after: Some(2),
        };
        let lifecycle = LifecycleChange {
            field: LifecycleChangeField::ReviewAfter,
            before: Some("2026-06-01".to_string()),
            after: Some("2026-07-01".to_string()),
        };
        let evidence = EvidenceChange {
            field: EvidenceChangeField::Evidence,
            removed: vec!["old receipt".to_string()],
            added: vec!["new receipt".to_string()],
        };
        let metadata = MetadataChange {
            field: MetadataChangeField::Reason,
            before: Some("old reason".to_string()),
            after: Some("new reason".to_string()),
        };
        let requirement = RequirementChange {
            field: RequirementChangeField::EvidenceRequired,
            before: false,
            after: true,
        };
        let policy_status = PolicyStatusChange {
            before: Some("audit".to_string()),
            after: Some("blocking".to_string()),
        };

        let change = base_change()
            .with_selector_precision(selector_precision.clone())
            .with_exception_identity(exception_identity.clone())
            .with_selector_identity(selector_identity.clone())
            .with_scope(scope.clone())
            .with_occurrence_limit(occurrence_limit.clone())
            .with_lifecycle(lifecycle.clone())
            .with_evidence(evidence.clone())
            .with_metadata(metadata.clone())
            .with_requirement(requirement.clone())
            .with_policy_status(policy_status.clone());

        assert_eq!(change.allow_id, "allow-test");
        assert_eq!(change.kind, PolicyChangeKind::ScopeChanged);
        assert_eq!(change.severity, PolicyChangeSeverity::Review);
        assert_eq!(change.message, "policy changed");
        assert_eq!(change.selector_precision, Some(selector_precision));
        assert_eq!(change.exception_identity, Some(exception_identity));
        assert_eq!(change.selector_identity, Some(selector_identity));
        assert_eq!(change.scope, Some(scope));
        assert_eq!(change.occurrence_limit, Some(occurrence_limit));
        assert_eq!(change.lifecycle, Some(lifecycle));
        assert_eq!(change.evidence, Some(evidence));
        assert_eq!(change.metadata, Some(metadata));
        assert_eq!(change.requirement, Some(requirement));
        assert_eq!(change.policy_status, Some(policy_status));
    }
}
