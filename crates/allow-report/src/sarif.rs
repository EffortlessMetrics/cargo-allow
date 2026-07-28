use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape, normalize_path};

use crate::evidence_repair::{
    evidence_repair_queues_from_context, push_evidence_repair_queue_json_fields,
};
use crate::json::{bool_json, option_json, push_json_source_context_properties};
use crate::{
    ReportContext, Summary, baseline_debt_count, broken_evidence_link_count,
    policy_missing_evidence_count, weak_evidence_reference_count,
};

pub fn render_sarif(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_sarif_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_sarif_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let reportable = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"$schema\": \"https://json.schemastore.org/sarif-2.1.0.json\",\n");
    out.push_str("  \"version\": \"2.1.0\",\n");
    out.push_str("  \"runs\": [\n");
    out.push_str("    {\n");
    out.push_str("      \"tool\": {\n");
    out.push_str("        \"driver\": {\n");
    out.push_str("          \"name\": \"cargo-allow\",\n");
    out.push_str(
        "          \"informationUri\": \"https://github.com/EffortlessMetrics/cargo-allow\",\n",
    );
    out.push_str("          \"rules\": [\n");
    for (index, status) in SARIF_STATUSES.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_sarif_rule(*status));
    }
    out.push_str("\n          ]\n");
    out.push_str("        }\n");
    out.push_str("      },\n");
    out.push_str("      \"properties\": {\n");
    out.push_str(&format!(
        "        \"command\": \"{}\",\n",
        json_escape(command)
    ));
    out.push_str(&format!(
        "        \"status\": \"{}\",\n",
        if failed { "failed" } else { "passed" }
    ));
    out.push_str(&format!("        \"failed\": {},\n", bool_json(failed)));
    push_json_source_context_properties(&mut out, context.into(), "        ");
    push_policy_context_properties(&mut out, &summary, context);
    push_evidence_repair_queues_property(&mut out, &summary, context);
    out.push_str("      },\n");
    out.push_str("      \"results\": [\n");
    for (index, outcome) in reportable.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
        out.push_str(&render_sarif_result(outcome, finding));
    }
    out.push_str("\n      ]\n");
    out.push_str("    }\n");
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

fn push_policy_context_properties(out: &mut String, summary: &Summary, context: ReportContext<'_>) {
    let rows = policy_context_property_rows(summary, context);
    if rows.is_empty() {
        return;
    }
    out.push_str(",\n");
    for (index, (name, count)) in rows.iter().enumerate() {
        let comma = if index + 1 == rows.len() { "" } else { "," };
        out.push_str(&format!("        \"{name}\": {count}{comma}\n"));
    }
}

fn policy_context_property_rows(
    summary: &Summary,
    context: ReportContext<'_>,
) -> Vec<(&'static str, usize)> {
    let mut rows = Vec::new();
    let baseline_debt = baseline_debt_count(summary, context);
    if baseline_debt > summary.count(MatchStatus::BaselineDebt) {
        rows.push(("policy_baseline_debt", baseline_debt));
    }
    let policy_missing_evidence = policy_missing_evidence_count(summary, context);
    if policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing) {
        rows.push(("policy_missing_evidence", policy_missing_evidence));
    }
    let broken_evidence_links = broken_evidence_link_count(context);
    if broken_evidence_links > 0 {
        rows.push(("broken_evidence_links", broken_evidence_links));
    }
    let weak_evidence_references = weak_evidence_reference_count(context);
    if weak_evidence_references > 0 {
        rows.push(("weak_evidence_references", weak_evidence_references));
    }
    rows
}

fn push_evidence_repair_queues_property(
    out: &mut String,
    summary: &Summary,
    context: ReportContext<'_>,
) {
    let queues = evidence_repair_queues_from_context(summary, context);
    if queues.is_empty() {
        return;
    }

    out.push_str(",\n");
    out.push_str("        \"evidence_repair_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("          {\n");
        push_evidence_repair_queue_json_fields(out, queue, "            ");
        out.push_str("          }");
    }
    out.push_str("\n        ]\n");
}

const SARIF_STATUSES: &[MatchStatus] = &[
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::ReviewDue,
    MatchStatus::LocationDrift,
    MatchStatus::Stale,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::MissingRequiredField,
    MatchStatus::EvidenceMissing,
    MatchStatus::BaselineDebt,
];

fn render_sarif_rule(status: MatchStatus) -> String {
    format!(
        "            {{\"id\": \"{}\", \"name\": \"{}\", \"shortDescription\": {{\"text\": \"{}\"}}}}",
        sarif_rule_id(status),
        status.as_str(),
        sarif_rule_description(status)
    )
}

fn render_sarif_result(outcome: &MatchOutcome, finding: Option<&Finding>) -> String {
    let mut out = String::new();
    out.push_str("        {\n");
    out.push_str(&format!(
        "          \"ruleId\": \"{}\",\n",
        sarif_rule_id(outcome.status)
    ));
    out.push_str(&format!(
        "          \"level\": \"{}\",\n",
        sarif_level(outcome.status)
    ));
    out.push_str(&format!(
        "          \"message\": {{\"text\": \"{}\"}},\n",
        json_escape(&outcome.message)
    ));
    out.push_str("          \"properties\": {\n");
    out.push_str(&format!(
        "            \"status\": \"{}\",\n",
        outcome.status.as_str()
    ));
    out.push_str(&format!(
        "            \"allow_id\": {},\n",
        option_json(outcome.allow_id.as_deref())
    ));
    out.push_str(&format!(
        "            \"finding_index\": {},\n",
        outcome
            .finding_index
            .map(|idx| idx.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!("            \"score\": {},\n", outcome.score));
    out.push_str(&format!(
        "            \"source_package\": {}\n",
        option_json(finding.and_then(|finding| finding.identity.crate_name.as_deref()))
    ));
    out.push_str("          }");
    if let Some(finding) = finding {
        out.push_str(",\n");
        out.push_str("          \"locations\": [\n");
        out.push_str(&render_sarif_location(finding));
        out.push_str("\n          ]\n");
        out.push_str("        }");
    } else {
        out.push('\n');
        out.push_str("        }");
    }
    out
}

fn render_sarif_location(finding: &Finding) -> String {
    let mut out = String::new();
    out.push_str("            {\n");
    out.push_str("              \"physicalLocation\": {\n");
    out.push_str(&format!(
        "                \"artifactLocation\": {{\"uri\": \"{}\"}}",
        json_escape(&normalize_path(&finding.path))
    ));
    if let Some(span) = &finding.span {
        out.push_str(",\n");
        out.push_str("                \"region\": {\n");
        out.push_str(&format!(
            "                  \"startLine\": {},\n",
            span.line
        ));
        out.push_str(&format!(
            "                  \"startColumn\": {}\n",
            span.column
        ));
        out.push_str("                }\n");
        out.push_str("              }\n");
    } else {
        out.push('\n');
        out.push_str("              }\n");
    }
    out.push_str("            }");
    out
}

fn sarif_rule_id(status: MatchStatus) -> String {
    format!("cargo-allow/{}", status.as_str())
}

fn sarif_rule_description(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::New => "New unreceipted source-tree exception finding.",
        MatchStatus::Expired => "Matched allow entry is expired.",
        MatchStatus::ReviewDue => "Matched allow entry is due for review.",
        MatchStatus::LocationDrift => "Matched allow entry moved since last_seen was recorded.",
        MatchStatus::Stale => "Allow entry did not match any current finding.",
        MatchStatus::Ambiguous => "Selector matched ambiguously and needs narrowing.",
        MatchStatus::InvalidSelector => "Allow entry selector is invalid.",
        MatchStatus::MissingRequiredField => "Allow entry is missing required policy metadata.",
        MatchStatus::EvidenceMissing => "Allow entry is missing required evidence.",
        MatchStatus::BaselineDebt => "Generated baseline debt remains in policy.",
        MatchStatus::Matched => "Finding matched policy.",
    }
}

fn sarif_level(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::New
        | MatchStatus::Expired
        | MatchStatus::Ambiguous
        | MatchStatus::InvalidSelector
        | MatchStatus::MissingRequiredField
        | MatchStatus::EvidenceMissing => "error",
        MatchStatus::ReviewDue | MatchStatus::BaselineDebt | MatchStatus::LocationDrift => {
            "warning"
        }
        MatchStatus::Stale => "note",
        MatchStatus::Matched => "none",
    }
}
