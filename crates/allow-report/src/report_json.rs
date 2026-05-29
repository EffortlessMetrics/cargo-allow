use crate::json::{
    bool_json, option_json, push_json_artifact_header, push_json_artifact_source_context,
    render_match_outcome_json_compact,
};
use crate::{
    REPORT_SCHEMA_ID, REPORT_SCHEMA_VERSION, ReportContext, Summary, baseline_debt_count,
    render_counts_fields_with_policy_baseline, review_item_count_with_baseline,
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
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    push_json_artifact_header(&mut out, REPORT_SCHEMA_VERSION, REPORT_SCHEMA_ID, command);
    out.push_str(&format!(
        "  \"status\": \"{}\",\n",
        if failed { "failed" } else { "passed" }
    ));
    out.push_str(&format!("  \"failed\": {},\n", bool_json(failed)));
    push_json_artifact_source_context(&mut out, context.into());
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"findings\": {},\n", findings.len()));
    out.push_str(&format!("    \"outcomes\": {},\n", summary.total));
    out.push_str(&render_counts_fields_with_policy_baseline(
        &summary,
        context.baseline_debt_entries,
        context.broken_evidence_links,
        "    ",
    ));
    out.push_str("  },\n");
    out.push_str("  \"trend\": {\n");
    out.push_str(&render_trend_fields(&summary, context, "    "));
    out.push_str("  },\n");
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
        out.push_str(&format!("\"kind\": \"{}\", ", finding.kind.as_str()));
        out.push_str(&format!(
            "\"family\": {}, ",
            option_json(finding.family.as_deref())
        ));
        out.push_str(&format!(
            "\"path\": \"{}\", ",
            json_escape(&normalize_path(&finding.path))
        ));
        out.push_str(&format!(
            "\"line\": {}, ",
            finding
                .span
                .as_ref()
                .map(|s| s.line.to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
        out.push_str(&format!(
            "\"container\": {}, ",
            option_json(finding.identity.container.as_deref())
        ));
        out.push_str(&format!(
            "\"source_package\": {}, ",
            option_json(finding.identity.crate_name.as_deref())
        ));
        out.push_str(&format!(
            "\"ast_kind\": \"{}\"",
            json_escape(&finding.identity.ast_kind)
        ));
        out.push('}');
    }
    out.push_str("\n  ]\n}");
    out
}

fn render_trend_fields(summary: &Summary, context: ReportContext<'_>, indent: &str) -> String {
    let baseline_debt = baseline_debt_count(summary, context);
    let broken_evidence_links = crate::broken_evidence_link_count(context);
    let mut fields = vec![
        (
            "review_items",
            review_item_count_with_baseline(summary, baseline_debt, broken_evidence_links),
        ),
        ("new", summary.count(MatchStatus::New)),
        ("expired", summary.count(MatchStatus::Expired)),
        ("review_due", summary.count(MatchStatus::ReviewDue)),
        ("stale", summary.count(MatchStatus::Stale)),
        ("ambiguous", summary.count(MatchStatus::Ambiguous)),
        (
            "invalid_selector",
            summary.count(MatchStatus::InvalidSelector),
        ),
        (
            "missing_required_field",
            summary.count(MatchStatus::MissingRequiredField),
        ),
        (
            "evidence_missing",
            summary.count(MatchStatus::EvidenceMissing),
        ),
        ("baseline_debt", baseline_debt),
    ];
    if broken_evidence_links > 0 {
        fields.push(("broken_evidence_links", broken_evidence_links));
    }
    fields
        .iter()
        .enumerate()
        .map(|(idx, (name, value))| {
            let comma = if idx + 1 == fields.len() { "" } else { "," };
            format!("{indent}\"{name}\": {value}{comma}\n")
        })
        .collect()
}
