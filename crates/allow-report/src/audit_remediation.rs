use allow_core::MatchStatus;

use crate::evidence_repair::{
    BROKEN_EVIDENCE_LINK_COMMAND, MISSING_EVIDENCE_COMMAND, WEAK_EVIDENCE_REFERENCE_COMMAND,
};
use crate::{ReviewSignals, Summary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuditRemediationItem {
    pub(crate) signal: &'static str,
    pub(crate) label: &'static str,
    pub(crate) count: usize,
    pub(crate) command: &'static str,
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
        "cargo-allow worklist --status new --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::Expired),
        "expired",
        "expired",
        "cargo-allow worklist --status expired --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::ReviewDue),
        "review_due",
        "review due",
        "cargo-allow worklist --status review_due --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::Stale),
        "stale",
        "stale",
        "cargo-allow prune --stale --dry-run --format json --output target/cargo-allow/prune.json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::Ambiguous),
        "ambiguous",
        "ambiguous",
        "cargo-allow worklist --status ambiguous --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::InvalidSelector),
        "invalid_selector",
        "invalid selectors",
        "cargo-allow worklist --status invalid_selector --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        summary.count(MatchStatus::MissingRequiredField),
        "missing_required_field",
        "missing required fields",
        "cargo-allow worklist --status missing_required_field --format json",
    );
    push_audit_remediation_item_if(
        &mut items,
        signals
            .policy_missing_evidence
            .max(summary.count(MatchStatus::EvidenceMissing)),
        "missing_evidence",
        "missing evidence",
        MISSING_EVIDENCE_COMMAND,
    );
    push_audit_remediation_item_if(
        &mut items,
        signals.broken_evidence_links,
        "broken_evidence_links",
        "broken evidence links",
        BROKEN_EVIDENCE_LINK_COMMAND,
    );
    push_audit_remediation_item_if(
        &mut items,
        signals.weak_evidence_references,
        "weak_evidence_references",
        "weak evidence references",
        WEAK_EVIDENCE_REFERENCE_COMMAND,
    );
    push_audit_remediation_item_if(
        &mut items,
        signals.baseline_debt,
        "baseline_debt",
        "baseline debt",
        "cargo-allow worklist --baseline-debt --format json",
    );
    items
}

fn push_audit_remediation_item_if(
    items: &mut Vec<AuditRemediationItem>,
    count: usize,
    signal: &'static str,
    label: &'static str,
    command: &'static str,
) {
    if count > 0 {
        items.push(AuditRemediationItem {
            signal,
            label,
            count,
            command,
        });
    }
}
