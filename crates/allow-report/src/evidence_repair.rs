use allow_core::MatchStatus;

use crate::{ReportContext, ReviewSignals, Summary};

pub(crate) const BROKEN_EVIDENCE_LINK_COMMAND: &str =
    "cargo-allow worklist --item-kind broken_evidence_link --format json";
pub(crate) const MISSING_EVIDENCE_COMMAND: &str =
    "cargo-allow worklist --missing-evidence --format json";
pub(crate) const WEAK_EVIDENCE_REFERENCE_COMMAND: &str =
    "cargo-allow worklist --item-kind weak_evidence_reference --format json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvidenceRepairQueue {
    pub(crate) signal: &'static str,
    pub(crate) label: &'static str,
    pub(crate) item_kind: Option<&'static str>,
    pub(crate) count: usize,
    pub(crate) command: &'static str,
}

pub(crate) fn evidence_repair_queues_from_context(
    summary: &Summary,
    context: ReportContext<'_>,
) -> Vec<EvidenceRepairQueue> {
    evidence_repair_queues(summary, ReviewSignals::from_summary(summary, context))
}

pub(crate) fn evidence_repair_queues(
    summary: &Summary,
    signals: ReviewSignals,
) -> Vec<EvidenceRepairQueue> {
    evidence_repair_queues_from_counts(
        signals.broken_evidence_links,
        signals
            .policy_missing_evidence
            .max(summary.count(MatchStatus::EvidenceMissing)),
        signals.weak_evidence_references,
    )
}

pub(crate) fn evidence_repair_queues_from_counts(
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
) -> Vec<EvidenceRepairQueue> {
    let mut queues = Vec::new();
    push_evidence_repair_queue_if(
        &mut queues,
        broken_evidence_links,
        "broken_evidence_links",
        "broken evidence links",
        Some("broken_evidence_link"),
        BROKEN_EVIDENCE_LINK_COMMAND,
    );
    push_evidence_repair_queue_if(
        &mut queues,
        missing_evidence,
        "missing_evidence",
        "missing evidence",
        None,
        MISSING_EVIDENCE_COMMAND,
    );
    push_evidence_repair_queue_if(
        &mut queues,
        weak_evidence_references,
        "weak_evidence_references",
        "weak evidence references",
        Some("weak_evidence_reference"),
        WEAK_EVIDENCE_REFERENCE_COMMAND,
    );
    queues
}

fn push_evidence_repair_queue_if(
    queues: &mut Vec<EvidenceRepairQueue>,
    count: usize,
    signal: &'static str,
    label: &'static str,
    item_kind: Option<&'static str>,
    command: &'static str,
) {
    if count > 0 {
        queues.push(EvidenceRepairQueue {
            signal,
            label,
            item_kind,
            count,
            command,
        });
    }
}
