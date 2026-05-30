use allow_core::AllowEntry;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};
use crate::policy_entry_evidence::evidence_policy_changes;
use crate::policy_entry_identity::identity_policy_changes;
use crate::policy_entry_lifecycle::lifecycle_policy_changes;
use crate::policy_entry_limits::occurrence_limit_policy_changes;
use crate::policy_entry_metadata::metadata_policy_changes;
use crate::policy_entry_scope::scope_policy_changes;
use crate::policy_entry_selector::selector_policy_changes;

pub(crate) fn added_allow_change(entry: &AllowEntry) -> PolicyChange {
    let baseline = entry.classification == "baseline_debt";
    PolicyChange::new(
        entry.id.clone(),
        if baseline {
            PolicyChangeKind::BaselineDebtAdded
        } else {
            PolicyChangeKind::AddedAllow
        },
        if baseline {
            PolicyChangeSeverity::Fail
        } else {
            PolicyChangeSeverity::Review
        },
        if baseline {
            format!("{} added generated baseline debt", entry.id)
        } else {
            format!("{} added a new allow entry", entry.id)
        },
    )
}

pub(crate) fn removed_allow_change(entry: &AllowEntry) -> PolicyChange {
    PolicyChange::new(
        entry.id.clone(),
        PolicyChangeKind::RemovedAllow,
        PolicyChangeSeverity::Improvement,
        format!("{} removed an allow entry", entry.id),
    )
}

pub(crate) fn entry_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    changes.extend(identity_policy_changes(base, head));
    changes.extend(scope_policy_changes(base, head));
    changes.extend(selector_policy_changes(base, head));
    changes.extend(lifecycle_policy_changes(base, head));
    changes.extend(evidence_policy_changes(base, head));
    changes.extend(metadata_policy_changes(base, head));
    changes.extend(occurrence_limit_policy_changes(base, head));
    changes
}
