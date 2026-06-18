use allow_core::{MatchOutcome, MatchStatus};

use crate::audit_remediation::{
    AuditRemediationItem, AuditRemediationRoute, audit_remediation_items,
};
use crate::{ReviewSignals, Summary};

fn outcome(status: MatchStatus) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: None,
        finding_index: None,
        message: status.as_str().to_string(),
        score: 0,
    }
}

fn empty_signals() -> ReviewSignals {
    ReviewSignals {
        baseline_debt: 0,
        policy_missing_evidence: 0,
        broken_evidence_links: 0,
        weak_evidence_references: 0,
        occurrence_headroom: 0,
        review_items: 0,
    }
}

#[test]
fn worklist_status_call_presence_observer() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::New)]);
    let items = audit_remediation_items(&summary, empty_signals());

    assert_eq!(
        items,
        vec![AuditRemediationItem {
            signal: "new_unreceipted",
            label: "new unreceipted",
            route: AuditRemediationRoute {
                route_kind: "worklist_status",
                item_kind: Some("new_unreceipted_finding"),
                worklist_status: Some("new"),
                worklist_filter: None,
            },
            count: 1,
            command: "cargo-allow worklist --status new --format json",
        }]
    );
}

#[test]
fn prune_stale_call_presence_observer() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Stale)]);
    let items = audit_remediation_items(&summary, empty_signals());

    assert_eq!(
        items,
        vec![AuditRemediationItem {
            signal: "stale",
            label: "stale",
            route: AuditRemediationRoute {
                route_kind: "prune_stale",
                item_kind: Some("stale_allow"),
                worklist_status: None,
                worklist_filter: None,
            },
            count: 1,
            command: "cargo-allow prune --stale --dry-run --format json --output target/cargo-allow/prune.json",
        }]
    );
}

#[test]
fn push_audit_remediation_item_if_call_presence_observer() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Expired)]);
    let items = audit_remediation_items(&summary, empty_signals());

    assert_eq!(
        items,
        vec![AuditRemediationItem {
            signal: "expired",
            label: "expired",
            route: AuditRemediationRoute {
                route_kind: "worklist_status",
                item_kind: Some("expired_allow"),
                worklist_status: Some("expired"),
                worklist_filter: None,
            },
            count: 1,
            command: "cargo-allow worklist --status expired --format json",
        }]
    );

    let empty = audit_remediation_items(&Summary::default(), empty_signals());
    assert_eq!(empty, Vec::new());
}
