use allow_core::AllowEntry;

use crate::policy_change::{
    OccurrenceLimitChange, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
};
use crate::policy_compare::{occurrence_limit_loosened, occurrence_limit_tightened};

pub(crate) fn occurrence_limit_policy_changes(
    base: &AllowEntry,
    head: &AllowEntry,
) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if occurrence_limit_loosened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            base,
            head,
            PolicyChangeKind::OccurrenceLimitLoosened,
            PolicyChangeSeverity::Fail,
            "occurrence_limit increased or removed",
        ));
    }
    if occurrence_limit_tightened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            base,
            head,
            PolicyChangeKind::OccurrenceLimitTightened,
            PolicyChangeSeverity::Improvement,
            "occurrence_limit tightened",
        ));
    }
    changes
}

fn change(
    base: &AllowEntry,
    entry: &AllowEntry,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
) -> PolicyChange {
    PolicyChange::new(
        entry.id.clone(),
        kind,
        severity,
        format!("{} {message}", entry.id),
    )
    .with_occurrence_limit(OccurrenceLimitChange {
        before: base.occurrence_limit,
        after: entry.occurrence_limit,
    })
}
