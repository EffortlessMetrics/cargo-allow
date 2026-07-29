use crate::allow_entry_json::push_optional_string_field;
use crate::audit_remediation::audit_remediation_items;
use crate::contracts::REPORT_ARTIFACT;
use crate::evidence_repair::{evidence_repair_queues, push_evidence_repair_queue_json_fields};
use crate::json::{
    option_json, push_json_artifact_header, push_json_artifact_source_context,
    push_json_status_fields, render_match_outcome_json_compact,
};
use crate::{
    DiffReport, REPORT_COMMAND_DIFF, REPORT_COMMANDS, ReportContext, ReviewSignals, Summary,
    render_advisory_count_fields, render_count_fields_with_policy_context,
};
use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape, normalize_path};

pub fn render_json(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_json_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_json_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    render_json_report(command, findings, outcomes, failed, context, None)
}

pub fn render_json_with_context_and_diff(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
    diff: DiffReport<'_>,
) -> String {
    assert_eq!(
        command, REPORT_COMMAND_DIFF,
        "diff report artifacts support only diff command"
    );
    render_json_report(command, findings, outcomes, failed, context, Some(diff))
}

fn render_json_report(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
    diff: Option<DiffReport<'_>>,
) -> String {
    assert!(
        REPORT_COMMANDS.contains(&command),
        "report artifacts support only audit, check, or diff commands"
    );
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    push_json_artifact_header(&mut out, REPORT_ARTIFACT, command);
    push_json_status_fields(&mut out, failed);
    push_json_artifact_source_context(&mut out, context.into());
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"findings\": {},\n", findings.len()));
    out.push_str(&format!("    \"outcomes\": {},\n", summary.total));
    out.push_str(&render_count_fields_with_policy_context(
        &summary,
        context.baseline_debt_entries,
        context.policy_missing_evidence_entries,
        context.broken_evidence_links,
        context.weak_evidence_references,
        "    ",
    ));
    out.push_str("  },\n");
    out.push_str("  \"trend\": {\n");
    out.push_str(&render_advisory_count_fields(&summary, context, "    "));
    out.push_str("  },\n");
    append_audit_remediation_roadmap_json(command, &summary, context, &mut out);
    append_evidence_repair_queues_json(&summary, context, &mut out);
    if let Some(source_inventory) = crate::render_source_inventory_json(findings, outcomes, "  ") {
        out.push_str("  \"source_inventory\": ");
        out.push_str(&source_inventory);
        out.push_str(",\n");
    }
    out.push_str("  \"outcomes\": [\n");
    for (i, outcome) in outcomes.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str("    ");
        out.push_str(&render_match_outcome_json_compact(outcome));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"findings\": [\n");
    for (i, finding) in findings.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {");
        let mut finding_fields = vec![format!(
            "\"kind\": \"{}\"",
            json_escape(finding.kind.as_str())
        )];
        push_optional_string_field(&mut finding_fields, "", "family", finding.family.as_deref());
        finding_fields.extend([
            format!(
                "\"path\": \"{}\"",
                json_escape(&normalize_path(&finding.path))
            ),
            format!(
                "\"line\": {}",
                finding
                    .span
                    .as_ref()
                    .map(|s| s.line.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ),
            format!(
                "\"container\": {}",
                option_json(finding.identity.container.as_deref())
            ),
        ]);
        push_optional_string_field(
            &mut finding_fields,
            "",
            "source_package",
            finding.identity.crate_name.as_deref(),
        );
        finding_fields.push(format!(
            "\"ast_kind\": \"{}\"",
            json_escape(&finding.identity.ast_kind)
        ));
        out.push_str(&finding_fields.join(", "));
        out.push('}');
    }
    match diff {
        Some(diff) => {
            out.push_str("\n  ],\n");
            out.push_str("  \"diff\": ");
            out.push_str(
                &crate::diff_json::render_diff_posture_json_with_evidence_health(
                    diff,
                    context.broken_evidence_links.unwrap_or(0),
                    context
                        .policy_missing_evidence_entries
                        .unwrap_or(0)
                        .max(summary.count(MatchStatus::EvidenceMissing)),
                    context.weak_evidence_references.unwrap_or(0),
                ),
            );
            out.push_str("\n}\n");
        }
        None => out.push_str("\n  ]\n}"),
    }
    out
}

fn append_evidence_repair_queues_json(
    summary: &Summary,
    context: ReportContext<'_>,
    out: &mut String,
) {
    let queues = evidence_repair_queues(summary, ReviewSignals::from_summary(summary, context));
    // #1858: always emit the array (even when empty) so downstream consumers
    // can distinguish "feature off" from "zero count" without per-artifact
    // special-casing.
    out.push_str("  \"evidence_repair_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        push_evidence_repair_queue_json_fields(out, queue, "      ");
        out.push_str("    }");
    }
    out.push_str("\n  ],\n");
}

fn append_audit_remediation_roadmap_json(
    command: &str,
    summary: &Summary,
    context: ReportContext<'_>,
    out: &mut String,
) {
    if command != "audit" {
        return;
    }
    let roadmap = audit_remediation_items(summary, ReviewSignals::from_summary(summary, context));
    if roadmap.is_empty() {
        return;
    }

    out.push_str("  \"audit_remediation_roadmap\": [\n");
    for (index, item) in roadmap.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"signal\": \"{}\",\n",
            json_escape(item.signal)
        ));
        out.push_str(&format!(
            "      \"label\": \"{}\",\n",
            json_escape(item.label)
        ));
        out.push_str(&format!(
            "      \"route_kind\": \"{}\",\n",
            json_escape(item.route.route_kind)
        ));
        if let Some(item_kind) = item.route.item_kind {
            out.push_str(&format!(
                "      \"item_kind\": \"{}\",\n",
                json_escape(item_kind)
            ));
        }
        if let Some(worklist_status) = item.route.worklist_status {
            out.push_str(&format!(
                "      \"worklist_status\": \"{}\",\n",
                json_escape(worklist_status)
            ));
        }
        if let Some(worklist_filter) = item.route.worklist_filter {
            out.push_str(&format!(
                "      \"worklist_filter\": \"{}\",\n",
                json_escape(worklist_filter)
            ));
        }
        out.push_str(&format!("      \"count\": {},\n", item.count));
        out.push_str(&format!(
            "      \"command\": \"{}\"\n",
            json_escape(item.command)
        ));
        out.push_str("    }");
    }
    out.push_str("\n  ],\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_remediation_json_returns_for_non_audit_and_clean_audit() {
        let summary = Summary::from_outcomes(&[test_outcome(MatchStatus::New)]);
        let mut non_audit = String::new();

        append_audit_remediation_roadmap_json(
            "check",
            &summary,
            ReportContext::default(),
            &mut non_audit,
        );

        assert_eq!(non_audit, "");

        let clean_summary = Summary::default();
        let mut clean_audit = String::new();
        append_audit_remediation_roadmap_json(
            "audit",
            &clean_summary,
            ReportContext::default(),
            &mut clean_audit,
        );

        assert_eq!(clean_audit, "");
    }

    #[test]
    fn audit_remediation_json_writes_multiple_route_shapes() {
        let outcomes = [
            test_outcome(MatchStatus::New),
            test_outcome(MatchStatus::Stale),
            test_outcome(MatchStatus::EvidenceMissing),
        ];
        let summary = Summary::from_outcomes(&outcomes);
        let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(2));
        context.policy_missing_evidence_entries = Some(4);
        context.broken_evidence_links = Some(1);
        context.weak_evidence_references = Some(3);
        let mut out = String::new();

        append_audit_remediation_roadmap_json("audit", &summary, context, &mut out);

        assert!(out.starts_with("  \"audit_remediation_roadmap\": [\n"));
        assert!(out.ends_with("\n  ],\n"));
        assert!(out.contains("    },\n    {\n"));
        assert!(out.contains("\"signal\": \"new_unreceipted\""));
        assert!(out.contains("\"route_kind\": \"worklist_status\""));
        assert!(out.contains("\"item_kind\": \"new_unreceipted_finding\""));
        assert!(out.contains("\"worklist_status\": \"new\""));
        assert!(out.contains("\"signal\": \"stale\""));
        assert!(out.contains("\"route_kind\": \"prune_stale\""));
        assert!(out.contains("\"item_kind\": \"stale_allow\""));
        assert!(out.contains("\"signal\": \"missing_evidence\""));
        assert!(out.contains("\"route_kind\": \"worklist_filter\""));
        assert!(out.contains("\"worklist_filter\": \"missing_evidence\""));
        assert!(out.contains("\"count\": 4"));
        assert!(out.contains("cargo-allow worklist --missing-evidence --format json"));
        assert!(out.contains("\"signal\": \"broken_evidence_links\""));
        assert!(out.contains("\"count\": 1"));
        assert!(out.contains("\"signal\": \"weak_evidence_references\""));
        assert!(out.contains("\"count\": 3"));
        assert!(out.contains("\"signal\": \"baseline_debt\""));
        assert!(out.contains("\"count\": 2"));
    }

    fn test_outcome(status: MatchStatus) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: None,
            candidate_ids: Vec::new(),
            finding_index: None,
            message: status.as_str().to_string(),
            score: 100,
        }
    }
}
