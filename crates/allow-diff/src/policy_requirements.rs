use allow_core::Requirements;

use crate::policy_change::{
    PolicyChange, PolicyChangeKind, PolicyChangeSeverity, RequirementChange, RequirementChangeField,
};

pub(crate) fn requirement_policy_changes(
    base: &Requirements,
    head: &Requirements,
) -> Vec<PolicyChange> {
    requirement_fields(base, head)
        .into_iter()
        .filter_map(requirement_change)
        .collect()
}

fn requirement_change(field: RequirementField) -> Option<PolicyChange> {
    if field.base == field.head {
        return None;
    }
    let loosened = if field.true_is_strict {
        field.base && !field.head
    } else {
        !field.base && field.head
    };
    let (kind, severity, direction) = if loosened {
        (
            PolicyChangeKind::RequirementLoosened,
            PolicyChangeSeverity::Fail,
            "loosened",
        )
    } else {
        (
            PolicyChangeKind::RequirementTightened,
            PolicyChangeSeverity::Improvement,
            "tightened",
        )
    };
    Some(
        PolicyChange::new(
            format!("requirements.{}", field.name.as_str()),
            kind,
            severity,
            format!(
                "requirements.{} {direction}: {} -> {}",
                field.name.as_str(),
                field.base,
                field.head
            ),
        )
        .with_requirement(RequirementChange {
            field: field.name,
            before: field.base,
            after: field.head,
        }),
    )
}

struct RequirementField {
    name: RequirementChangeField,
    base: bool,
    head: bool,
    true_is_strict: bool,
}

fn requirement_fields(base: &Requirements, head: &Requirements) -> Vec<RequirementField> {
    vec![
        field(
            RequirementChangeField::OwnerRequired,
            base.owner_required,
            head.owner_required,
            true,
        ),
        field(
            RequirementChangeField::ReasonRequired,
            base.reason_required,
            head.reason_required,
            true,
        ),
        field(
            RequirementChangeField::ClassificationRequired,
            base.classification_required,
            head.classification_required,
            true,
        ),
        field(
            RequirementChangeField::EvidenceRequired,
            base.evidence_required,
            head.evidence_required,
            true,
        ),
        field(
            RequirementChangeField::ExpiresOrReviewAfterRequired,
            base.expires_or_review_after_required,
            head.expires_or_review_after_required,
            true,
        ),
        field(
            RequirementChangeField::AllowBareAllowAttributes,
            base.allow_bare_allow_attributes,
            head.allow_bare_allow_attributes,
            false,
        ),
        field(
            RequirementChangeField::LintPolicyIdRequired,
            base.lint_policy_id_required,
            head.lint_policy_id_required,
            true,
        ),
        field(
            RequirementChangeField::StaleEntriesFail,
            base.stale_entries_fail,
            head.stale_entries_fail,
            true,
        ),
        field(
            RequirementChangeField::UnsafeEvidenceRequired,
            base.unsafe_evidence_required,
            head.unsafe_evidence_required,
            true,
        ),
        field(
            RequirementChangeField::UnsafeVerifiedEvidenceRequired,
            base.unsafe_verified_evidence_required,
            head.unsafe_verified_evidence_required,
            true,
        ),
        field(
            RequirementChangeField::UnsafeSafetyCommentRequired,
            base.unsafe_safety_comment_required,
            head.unsafe_safety_comment_required,
            true,
        ),
    ]
}

fn field(
    name: RequirementChangeField,
    base: bool,
    head: bool,
    true_is_strict: bool,
) -> RequirementField {
    RequirementField {
        name,
        base,
        head,
        true_is_strict,
    }
}
