use allow_core::MatchStatus;

use crate::evidence_repair::{
    BROKEN_EVIDENCE_LINK_COMMAND, EvidenceRepairQueue, MISSING_EVIDENCE_COMMAND,
    WEAK_EVIDENCE_REFERENCE_COMMAND, evidence_repair_queues_from_context,
    push_evidence_repair_queue_json_fields,
};
use crate::{ReportContext, Summary};

#[test]
fn evidence_repair_queues_from_context_routes_all_signal_counts() {
    let mut summary = Summary::default();
    summary.by_status.insert(MatchStatus::EvidenceMissing, 2);
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(3);
    context.policy_missing_evidence_entries = Some(5);
    context.weak_evidence_references = Some(7);

    let queues = evidence_repair_queues_from_context(&summary, context);

    assert_eq!(queues.len(), 3);
    assert_eq!(
        queues[0],
        EvidenceRepairQueue {
            signal: "broken_evidence_links",
            label: "broken evidence links",
            route_kind: "worklist_filter",
            item_kind: Some("broken_evidence_link"),
            worklist_filter: Some("broken_evidence"),
            count: 3,
            command: BROKEN_EVIDENCE_LINK_COMMAND,
        }
    );
    assert_eq!(
        queues[1],
        EvidenceRepairQueue {
            signal: "missing_evidence",
            label: "missing evidence",
            route_kind: "worklist_filter",
            item_kind: Some("missing_evidence"),
            worklist_filter: Some("missing_evidence"),
            count: 5,
            command: MISSING_EVIDENCE_COMMAND,
        }
    );
    assert_eq!(
        queues[2],
        EvidenceRepairQueue {
            signal: "weak_evidence_references",
            label: "weak evidence references",
            route_kind: "worklist_filter",
            item_kind: Some("weak_evidence_reference"),
            worklist_filter: Some("weak_evidence"),
            count: 7,
            command: WEAK_EVIDENCE_REFERENCE_COMMAND,
        }
    );
}

#[test]
fn push_evidence_repair_queue_json_fields_emits_full_queue_fragment() {
    let queue = EvidenceRepairQueue {
        signal: "broken_evidence_links",
        label: "broken \"evidence\" links",
        route_kind: "worklist_filter",
        item_kind: Some("broken_evidence_link"),
        worklist_filter: Some("broken_evidence"),
        count: 3,
        command: "cargo-allow worklist --broken-evidence --format \"json\"",
    };
    let mut out = String::new();

    push_evidence_repair_queue_json_fields(&mut out, &queue, "  ");

    assert_eq!(
        out,
        concat!(
            "  \"signal\": \"broken_evidence_links\",\n",
            "  \"label\": \"broken \\\"evidence\\\" links\",\n",
            "  \"route_kind\": \"worklist_filter\",\n",
            "  \"item_kind\": \"broken_evidence_link\",\n",
            "  \"worklist_filter\": \"broken_evidence\",\n",
            "  \"count\": 3,\n",
            "  \"command\": \"cargo-allow worklist --broken-evidence --format \\\"json\\\"\"\n"
        )
    );
}

#[test]
fn push_evidence_repair_queue_json_fields_omits_absent_optional_fields() {
    let queue = EvidenceRepairQueue {
        signal: "custom_signal",
        label: "custom signal",
        route_kind: "manual",
        item_kind: None,
        worklist_filter: None,
        count: 1,
        command: "cargo-allow worklist --format json",
    };
    let mut out = String::new();

    push_evidence_repair_queue_json_fields(&mut out, &queue, "    ");

    assert_eq!(
        out,
        concat!(
            "    \"signal\": \"custom_signal\",\n",
            "    \"label\": \"custom signal\",\n",
            "    \"route_kind\": \"manual\",\n",
            "    \"count\": 1,\n",
            "    \"command\": \"cargo-allow worklist --format json\"\n"
        )
    );
    assert!(!out.contains("\"item_kind\""));
    assert!(!out.contains("\"worklist_filter\""));
}
