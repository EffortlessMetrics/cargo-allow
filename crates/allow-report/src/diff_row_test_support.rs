use allow_core::{PostureDelta, PresenceMovement};

use crate::{DiffFindingChange, DiffLedgerMovementSummary, DiffPolicyChange};

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

pub fn test_finding_change<'a>(
    change: &'a str,
    key: &'a str,
    kind: &'a str,
    path: &'a str,
) -> DiffFindingChange<'a> {
    let (movement, posture_delta) = match change {
        "new" => (
            PresenceMovement::Introduced.field_name(),
            PostureDelta::ReviewRequired.field_name(),
        ),
        "removed" => (
            PresenceMovement::Removed.field_name(),
            PostureDelta::Improved.field_name(),
        ),
        _ => (
            PresenceMovement::Retained.field_name(),
            PostureDelta::Unchanged.field_name(),
        ),
    };
    DiffFindingChange {
        change,
        movement,
        posture_delta,
            changed_in_diff: true,
            subject: None,
        allow_id: None,
            ledger_id: None,
            lane: None,
        key,
        kind,
        family: None,
        path,
        line: None,
        column: None,
        source_package: None,
        identity: None,
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

pub fn test_diff_report<'a>(
    net_posture: &'a str,
    reviewer_action: &'a str,
    summary: crate::DiffPostureSummary,
    finding_changes: &'a [DiffFindingChange<'a>],
    policy_changes: &'a [DiffPolicyChange<'a>],
) -> crate::DiffReport<'a> {
    crate::DiffReport {
        net_posture,
        reviewer_action,
        summary,
        ledger_movement: empty_ledger_movement_summary(),
        finding_changes,
        policy_changes,
    }
}
