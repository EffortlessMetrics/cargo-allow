use allow_core::{MatchOutcome, MatchStatus};
use serde_json::Value;

pub(crate) struct StructuredDiffFixture {
    pub(crate) outcomes: Vec<MatchOutcome>,
    pub(crate) finding_changes: Vec<allow_diff::FindingPostureChange>,
    pub(crate) policy_changes: Vec<allow_diff::PolicyChange>,
}

pub(crate) fn structured_diff_fixture() -> StructuredDiffFixture {
    StructuredDiffFixture {
        outcomes: vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap at src/lib.rs:1:1",
        )],
        finding_changes: vec![finding_posture_change(
            allow_diff::FindingPostureKind::New,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )],
        policy_changes: vec![
            allow_diff::PolicyChange {
                allow_id: "allow-0001".to_string(),
                kind: allow_diff::PolicyChangeKind::SelectorPrecisionDecreased,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "allow-0001 selector precision decreased: 80 -> 45".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: Some(allow_diff::SelectorPrecisionChange {
                    before: 80,
                    after: 45,
                    removed_fields: vec!["container", "normalized_snippet_hash"],
                    added_fields: vec![],
                }),
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0002".to_string(),
                kind: allow_diff::PolicyChangeKind::ScopeBroadened,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "allow-0002 scope broadened".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: Some(allow_diff::ScopeChange {
                    field: allow_diff::ScopeChangeField::Effective,
                    before: Some("src/lib.rs".to_string()),
                    after: Some("src/**".to_string()),
                }),
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0003".to_string(),
                kind: allow_diff::PolicyChangeKind::OccurrenceLimitLoosened,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "allow-0003 occurrence_limit increased or removed".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: Some(allow_diff::OccurrenceLimitChange {
                    before: Some(1),
                    after: None,
                }),
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0004".to_string(),
                kind: allow_diff::PolicyChangeKind::ExpiryExtended,
                severity: allow_diff::PolicyChangeSeverity::Review,
                message: "allow-0004 expiry extended or removed".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: Some(allow_diff::LifecycleChange {
                    field: allow_diff::LifecycleChangeField::Expires,
                    before: Some("2026-09-01".to_string()),
                    after: Some("2026-12-01".to_string()),
                }),
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0005".to_string(),
                kind: allow_diff::PolicyChangeKind::EvidenceRemoved,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "allow-0005 evidence removed".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: Some(allow_diff::EvidenceChange {
                    field: allow_diff::EvidenceChangeField::Evidence,
                    removed: vec!["test:old-proof".to_string()],
                    added: vec![],
                }),
                metadata: None,
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0006".to_string(),
                kind: allow_diff::PolicyChangeKind::OwnerRemoved,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "allow-0006 owner removed".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: Some(allow_diff::MetadataChange {
                    field: allow_diff::MetadataChangeField::Owner,
                    before: Some("core".to_string()),
                    after: None,
                }),
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "requirements.owner_required".to_string(),
                kind: allow_diff::PolicyChangeKind::RequirementLoosened,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "requirements.owner_required loosened: true -> false".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: Some(allow_diff::RequirementChange {
                    field: allow_diff::RequirementChangeField::OwnerRequired,
                    before: true,
                    after: false,
                }),
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "policy.status".to_string(),
                kind: allow_diff::PolicyChangeKind::PolicyStatusWeakened,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "policy.status weakened: active -> advisory".to_string(),
                exception_identity: None,
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: Some(allow_diff::PolicyStatusChange {
                    before: Some("active".to_string()),
                    after: Some("advisory".to_string()),
                }),
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0007".to_string(),
                kind: allow_diff::PolicyChangeKind::KindChanged,
                severity: allow_diff::PolicyChangeSeverity::Fail,
                message: "allow-0007 changed governed exception kind: panic -> unsafe".to_string(),
                exception_identity: Some(allow_diff::ExceptionIdentityChange {
                    field: allow_diff::ExceptionIdentityChangeField::Kind,
                    before: Some("panic".to_string()),
                    after: Some("unsafe".to_string()),
                }),
                selector_identity: None,
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: None,
            },
            allow_diff::PolicyChange {
                allow_id: "allow-0008".to_string(),
                kind: allow_diff::PolicyChangeKind::SelectorChanged,
                severity: allow_diff::PolicyChangeSeverity::Review,
                message: "allow-0008 selector identity changed".to_string(),
                exception_identity: None,
                selector_identity: Some(allow_diff::SelectorIdentityChange {
                    changed_fields: vec!["container", "normalized_snippet_hash"],
                }),
                selector_precision: None,
                scope: None,
                occurrence_limit: None,
                lifecycle: None,
                evidence: None,
                metadata: None,
                requirement: None,
                policy_status: None,
            },
        ],
    }
}

fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}

fn finding_posture_change(
    kind: allow_diff::FindingPostureKind,
    finding_kind: &str,
    family: Option<&str>,
    path: &str,
) -> allow_diff::FindingPostureChange {
    allow_diff::FindingPostureChange {
        kind,
        key: format!("{finding_kind}:{path}"),
        finding_kind: finding_kind.to_string(),
        family: family.map(str::to_string),
        path: path.to_string(),
        source_package: Some("parser".to_string()),
    }
}

pub(crate) fn parse_json(name: &str, json: &str) -> Value {
    match serde_json::from_str(json) {
        Ok(value) => value,
        Err(err) => std::panic::panic_any(format!("{name} should parse as JSON: {err}\n{json}")),
    }
}

pub(crate) fn first_array_item<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    let Some(items) = value.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{pointer} should be an array"));
    };
    let Some(item) = items.first() else {
        std::panic::panic_any(format!("{pointer} should contain at least one item"));
    };
    item
}
