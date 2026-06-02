use crate::contracts::RECEIPT_ARTIFACT;
use crate::json::{
    push_json_artifact_header, push_json_artifact_source_context, push_json_status_fields,
};
use crate::{
    RECEIPT_COMMAND_CHECK, ReportContext, Summary, baseline_debt_count,
    render_count_fields_with_policy_context, render_source_inventory_json,
};
use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape};

pub fn render_receipt(command: &str, outcomes: &[MatchOutcome], failed: bool) -> String {
    render_receipt_with_context(command, outcomes, failed, ReportContext::default())
}

pub fn render_receipt_with_context(
    command: &str,
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    render_receipt_json(command, None, outcomes, failed, context)
}

pub fn render_receipt_with_context_and_inventory(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    render_receipt_json(command, Some(findings), outcomes, failed, context)
}

fn render_receipt_json(
    command: &str,
    findings: Option<&[Finding]>,
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    assert_eq!(
        command, RECEIPT_COMMAND_CHECK,
        "receipt artifacts support only the check command"
    );
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    push_json_artifact_header(&mut out, RECEIPT_ARTIFACT, command);
    push_json_status_fields(&mut out, failed);
    push_json_artifact_source_context(&mut out, context.into());
    out.push_str("  \"counts\": {\n");
    out.push_str(&render_count_fields_with_policy_context(
        &summary,
        Some(baseline_debt_count(&summary, context)),
        context.policy_missing_evidence_entries,
        context.broken_evidence_links,
        context.weak_evidence_references,
        "    ",
    ));
    out.push_str("  }");
    append_evidence_repair_queues_json(&summary, context, &mut out);
    if let Some(source_inventory) =
        findings.and_then(|findings| render_source_inventory_json(findings, outcomes, "  "))
    {
        out.push_str(",\n  \"source_inventory\": ");
        out.push_str(&source_inventory);
        out.push('\n');
    } else {
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

fn append_evidence_repair_queues_json(
    summary: &Summary,
    context: ReportContext<'_>,
    out: &mut String,
) {
    let queues = evidence_repair_queues(summary, context);
    if queues.is_empty() {
        return;
    }

    out.push_str(",\n  \"evidence_repair_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"signal\": \"{}\",\n",
            json_escape(queue.signal)
        ));
        out.push_str(&format!("      \"count\": {},\n", queue.count));
        out.push_str(&format!(
            "      \"command\": \"{}\"\n",
            json_escape(queue.command)
        ));
        out.push_str("    }");
    }
    out.push_str("\n  ]");
}

fn evidence_repair_queues(
    summary: &Summary,
    context: ReportContext<'_>,
) -> Vec<EvidenceRepairQueue> {
    let mut queues = Vec::new();
    if let Some(count) = context.broken_evidence_links.filter(|count| *count > 0) {
        queues.push(EvidenceRepairQueue {
            signal: "broken_evidence_links",
            count,
            command: "cargo-allow worklist --item-kind broken_evidence_link --format json",
        });
    }
    let missing_evidence_count = context
        .policy_missing_evidence_entries
        .unwrap_or(0)
        .max(summary.count(MatchStatus::EvidenceMissing));
    if missing_evidence_count > 0 {
        queues.push(EvidenceRepairQueue {
            signal: "missing_evidence",
            count: missing_evidence_count,
            command: "cargo-allow worklist --missing-evidence --format json",
        });
    }
    if let Some(count) = context.weak_evidence_references.filter(|count| *count > 0) {
        queues.push(EvidenceRepairQueue {
            signal: "weak_evidence_references",
            count,
            command: "cargo-allow worklist --item-kind weak_evidence_reference --format json",
        });
    }
    queues
}

struct EvidenceRepairQueue {
    signal: &'static str,
    count: usize,
    command: &'static str,
}
