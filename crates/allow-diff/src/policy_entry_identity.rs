use allow_core::AllowEntry;

use crate::policy_change::{
    ExceptionIdentityChange, ExceptionIdentityChangeField, PolicyChange, PolicyChangeKind,
    PolicyChangeSeverity,
};

pub(crate) fn identity_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if base.kind != head.kind {
        changes.push(
            PolicyChange::new(
                head.id.clone(),
                PolicyChangeKind::KindChanged,
                PolicyChangeSeverity::Fail,
                format!(
                    "{} changed governed exception kind: {} -> {}",
                    head.id,
                    base.kind.as_str(),
                    head.kind.as_str()
                ),
            )
            .with_exception_identity(ExceptionIdentityChange {
                field: ExceptionIdentityChangeField::Kind,
                before: Some(base.kind.as_str().to_string()),
                after: Some(head.kind.as_str().to_string()),
            }),
        );
    }
    if base.family != head.family {
        changes.push(
            PolicyChange::new(
                head.id.clone(),
                PolicyChangeKind::FamilyChanged,
                PolicyChangeSeverity::Fail,
                format!(
                    "{} changed governed exception family: {} -> {}",
                    head.id,
                    base.family.as_deref().unwrap_or("<none>"),
                    head.family.as_deref().unwrap_or("<none>")
                ),
            )
            .with_exception_identity(ExceptionIdentityChange {
                field: ExceptionIdentityChangeField::Family,
                before: base.family.clone(),
                after: head.family.clone(),
            }),
        );
    }
    changes
}
