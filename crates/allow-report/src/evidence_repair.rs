use allow_core::{MatchStatus, json_escape};

use crate::{ReportContext, ReviewSignals, Summary};

pub(crate) const OCCURRENCE_HEADROOM_COMMAND: &str =
    "cargo-allow worklist --item-kind occurrence_headroom --format json";

pub(crate) const BROKEN_EVIDENCE_LINK_COMMAND: &str =
    "cargo-allow worklist --broken-evidence --format json";
pub(crate) const MISSING_EVIDENCE_COMMAND: &str =
    "cargo-allow worklist --missing-evidence --format json";
pub(crate) const WEAK_EVIDENCE_REFERENCE_COMMAND: &str =
    "cargo-allow worklist --weak-evidence --format json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvidenceRepairQueue {
    pub(crate) signal: &'static str,
    pub(crate) label: &'static str,
    pub(crate) route_kind: &'static str,
    pub(crate) item_kind: Option<&'static str>,
    pub(crate) worklist_filter: Option<&'static str>,
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
        signals.occurrence_headroom,
    )
}

pub(crate) fn evidence_repair_queues_from_counts(
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    occurrence_headroom: usize,
) -> Vec<EvidenceRepairQueue> {
    let mut queues = Vec::new();
    push_evidence_repair_queue_if(
        &mut queues,
        EvidenceRepairQueue {
            signal: "broken_evidence_links",
            label: "broken evidence links",
            route_kind: "worklist_filter",
            item_kind: Some("broken_evidence_link"),
            worklist_filter: Some("broken_evidence"),
            count: broken_evidence_links,
            command: BROKEN_EVIDENCE_LINK_COMMAND,
        },
    );
    push_evidence_repair_queue_if(
        &mut queues,
        EvidenceRepairQueue {
            signal: "missing_evidence",
            label: "missing evidence",
            route_kind: "worklist_filter",
            item_kind: Some("missing_evidence"),
            worklist_filter: Some("missing_evidence"),
            count: missing_evidence,
            command: MISSING_EVIDENCE_COMMAND,
        },
    );
    push_evidence_repair_queue_if(
        &mut queues,
        EvidenceRepairQueue {
            signal: "weak_evidence_references",
            label: "weak evidence references",
            route_kind: "worklist_filter",
            item_kind: Some("weak_evidence_reference"),
            worklist_filter: Some("weak_evidence"),
            count: weak_evidence_references,
            command: WEAK_EVIDENCE_REFERENCE_COMMAND,
        },
    );
    push_evidence_repair_queue_if(
        &mut queues,
        EvidenceRepairQueue {
            signal: "occurrence_headroom",
            label: "occurrence headroom",
            route_kind: "worklist_item_kind",
            item_kind: Some("occurrence_headroom"),
            worklist_filter: None,
            count: occurrence_headroom,
            command: OCCURRENCE_HEADROOM_COMMAND,
        },
    );
    queues
}

pub(crate) fn push_evidence_repair_queue_json_fields(
    out: &mut String,
    queue: &EvidenceRepairQueue,
    indent: &str,
) {
    out.push_str(&format!(
        "{indent}\"signal\": \"{}\",\n",
        json_escape(queue.signal)
    ));
    out.push_str(&format!(
        "{indent}\"label\": \"{}\",\n",
        json_escape(queue.label)
    ));
    out.push_str(&format!(
        "{indent}\"route_kind\": \"{}\",\n",
        json_escape(queue.route_kind)
    ));
    if let Some(item_kind) = queue.item_kind {
        out.push_str(&format!(
            "{indent}\"item_kind\": \"{}\",\n",
            json_escape(item_kind)
        ));
    }
    if let Some(worklist_filter) = queue.worklist_filter {
        out.push_str(&format!(
            "{indent}\"worklist_filter\": \"{}\",\n",
            json_escape(worklist_filter)
        ));
    }
    out.push_str(&format!("{indent}\"count\": {},\n", queue.count));
    out.push_str(&format!(
        "{indent}\"command\": \"{}\"\n",
        json_escape(queue.command)
    ));
}

fn push_evidence_repair_queue_if(
    queues: &mut Vec<EvidenceRepairQueue>,
    queue: EvidenceRepairQueue,
) {
    if queue.count > 0 {
        queues.push(queue);
    }
}
