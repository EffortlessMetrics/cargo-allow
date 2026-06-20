use allow_core::{PostureDelta, PresenceMovement};

use crate::{DiffLedgerMovementSummary, DiffPolicyChange};

pub fn empty_ledger_movement_summary() -> DiffLedgerMovementSummary {
    DiffLedgerMovementSummary {
        movement: crate::DiffMovementCounts {
            introduced: 0,
            retained: 0,
            removed: 0,
        },
        posture_delta: crate::DiffPostureDeltaCounts {
            improved: 0,
            worsened: 0,
            review_required: 0,
            unchanged: 0,
        },
    }
}

pub fn test_policy_change<'a>(
    severity: &'a str,
    allow_id: &'a str,
    kind: &'a str,
) -> DiffPolicyChange<'a> {
    let (movement, posture_delta) = match kind {
        "added_allow" | "baseline_debt_added" => (
            PresenceMovement::Introduced.field_name(),
            match severity {
                "fail" => PostureDelta::Worsened.field_name(),
                "review" => PostureDelta::ReviewRequired.field_name(),
                _ => PostureDelta::Improved.field_name(),
            },
        ),
        "removed_allow" => (
            PresenceMovement::Removed.field_name(),
            PostureDelta::Improved.field_name(),
        ),
        _ => (
            PresenceMovement::Retained.field_name(),
            match severity {
                "fail" => PostureDelta::Worsened.field_name(),
                "review" => PostureDelta::ReviewRequired.field_name(),
                _ => PostureDelta::Improved.field_name(),
            },
        ),
    };
    DiffPolicyChange {
        severity,
        movement,
        posture_delta,
        changed_in_diff: true,
        subject: Some(allow_id),
        allow_id,
        ledger_id: None,
        lane: None,
        kind,
        message: "policy changed",
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
