use allow_core::MatchStatus;

use crate::evidence_repair::{
    BROKEN_EVIDENCE_LINK_COMMAND, MISSING_EVIDENCE_COMMAND, WEAK_EVIDENCE_REFERENCE_COMMAND,
};
use crate::{ReviewSignals, Summary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuditRemediationItem {
    pub(crate) signal: &'static str,
    pub(crate) label: &'static str,
    pub(crate) route: AuditRemediationRoute,
    pub(crate) count: usize,
    pub(crate) command: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuditRemediationRoute {
    pub(crate) route_kind: &'static str,
    pub(crate) item_kind: Option<&'static str>,
    pub(crate) worklist_status: Option<&'static str>,
    pub(crate) worklist_filter: Option<&'static str>,
}

impl AuditRemediationRoute {
    fn worklist_status(item_kind: &'static str, worklist_status: &'static str) -> Self {
        Self {
            route_kind: "worklist_status",
            item_kind: Some(item_kind),
            worklist_status: Some(worklist_status),
            worklist_filter: None,
        }
    }

    fn worklist_filter(item_kind: &'static str, worklist_filter: &'static str) -> Self {
        Self {
            route_kind: "worklist_filter",
            item_kind: Some(item_kind),
            worklist_status: None,
            worklist_filter: Some(worklist_filter),
        }
    }

    fn prune_stale() -> Self {
        Self {
            route_kind: "prune_stale",
            item_kind: Some("stale_allow"),
            worklist_status: None,
            worklist_filter: None,
        }
    }
}

pub(crate) fn audit_remediation_items(
    summary: &Summary,
    signals: ReviewSignals,
) -> Vec<AuditRemediationItem> {
    let mut items = Vec::new();
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::New),
        "new_unreceipted",
        "new unreceipted",
        AuditRemediationRoute::worklist_status("new_unreceipted_finding", "new"),
        "cargo-allow worklist --status new --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::Expired),
        "expired",
        "expired",
        AuditRemediationRoute::worklist_status("expired_allow", "expired"),
        "cargo-allow worklist --status expired --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::ReviewDue),
        "review_due",
        "review due",
        AuditRemediationRoute::worklist_status("review_due", "review_due"),
        "cargo-allow worklist --status review_due --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::Stale),
        "stale",
        "stale",
        AuditRemediationRoute::prune_stale(),
        "cargo-allow prune --stale --dry-run --format json --output target/cargo-allow/prune.json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::Ambiguous),
        "ambiguous",
        "ambiguous",
        AuditRemediationRoute::worklist_status("ambiguous_selector", "ambiguous"),
        "cargo-allow worklist --status ambiguous --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::InvalidSelector),
        "invalid_selector",
        "invalid selectors",
        AuditRemediationRoute::worklist_status("invalid_selector", "invalid_selector"),
        "cargo-allow worklist --status invalid_selector --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::MissingRequiredField),
        "missing_required_field",
        "missing required fields",
        AuditRemediationRoute::worklist_status("missing_required_field", "missing_required_field"),
        "cargo-allow worklist --status missing_required_field --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        signals
            .policy_missing_evidence
            .max(summary.count(MatchStatus::EvidenceMissing)),
        "missing_evidence",
        "missing evidence",
        AuditRemediationRoute::worklist_filter("missing_evidence", "missing_evidence"),
        MISSING_EVIDENCE_COMMAND,
    );
    push_audit_remediation_item_if(
        &mut items,
        signals.broken_evidence_links,
        "broken_evidence_links",
        "broken evidence links",
        AuditRemediationRoute::worklist_filter("broken_evidence_link", "broken_evidence"),
        BROKEN_EVIDENCE_LINK_COMMAND,
    );
    push_audit_remediation_item_if(
        &mut items,
        signals.weak_evidence_references,
        "weak_evidence_references",
        "weak evidence references",
        AuditRemediationRoute::worklist_filter("weak_evidence_reference", "weak_evidence"),
        WEAK_EVIDENCE_REFERENCE_COMMAND,
    );
    push_audit_remediation_item_if(
        &mut items,
        signals.baseline_debt,
        "baseline_debt",
        "baseline debt",
        AuditRemediationRoute::worklist_filter("baseline_debt", "baseline_debt"),
        "cargo-allow worklist --baseline-debt --format json",
    );
    items
}

fn push_audit_remediation_item_if(
    items: &mut Vec<AuditRemediationItem>,
    count: usize,
    signal: &'static str,
    label: &'static str,
    route: AuditRemediationRoute,
    command: &'static str,
) {
    if count > 0 {
        items.push(AuditRemediationItem {
            signal,
            label,
            route,
            count,
            command,
        });
    }
}
