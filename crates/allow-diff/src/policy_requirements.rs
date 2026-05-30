use allow_core::Requirements;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

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
    Some(PolicyChange::new(
        format!("requirements.{}", field.name),
        kind,
        severity,
        format!(
            "requirements.{} {direction}: {} -> {}",
            field.name, field.base, field.head
        ),
    ))
}

struct RequirementField {
    name: &'static str,
    base: bool,
    head: bool,
    true_is_strict: bool,
}

fn requirement_fields(base: &Requirements, head: &Requirements) -> Vec<RequirementField> {
    vec![
        field(
            "owner_required",
            base.owner_required,
            head.owner_required,
            true,
        ),
        field(
            "reason_required",
            base.reason_required,
            head.reason_required,
            true,
        ),
        field(
            "classification_required",
            base.classification_required,
            head.classification_required,
            true,
        ),
        field(
            "evidence_required",
            base.evidence_required,
            head.evidence_required,
            true,
        ),
        field(
            "expires_or_review_after_required",
            base.expires_or_review_after_required,
            head.expires_or_review_after_required,
            true,
        ),
        field(
            "allow_bare_allow_attributes",
            base.allow_bare_allow_attributes,
            head.allow_bare_allow_attributes,
            false,
        ),
        field(
            "lint_policy_id_required",
            base.lint_policy_id_required,
            head.lint_policy_id_required,
            true,
        ),
        field(
            "stale_entries_fail",
            base.stale_entries_fail,
            head.stale_entries_fail,
            true,
        ),
        field(
            "unsafe.evidence_required",
            base.unsafe_evidence_required,
            head.unsafe_evidence_required,
            true,
        ),
        field(
            "unsafe.safety_comment_required",
            base.unsafe_safety_comment_required,
            head.unsafe_safety_comment_required,
            true,
        ),
    ]
}

fn field(name: &'static str, base: bool, head: bool, true_is_strict: bool) -> RequirementField {
    RequirementField {
        name,
        base,
        head,
        true_is_strict,
    }
}
