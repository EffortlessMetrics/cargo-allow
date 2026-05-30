use crate::contracts::REPORT_ARTIFACT;
use crate::json::{
    option_json, push_json_artifact_header, push_json_artifact_source_context,
    push_json_status_fields, render_match_outcome_json_compact,
};
use crate::{
    DiffReport, REPORT_COMMAND_DIFF, REPORT_COMMANDS, ReportContext, ReviewSignals, Summary,
    render_count_fields_with_policy_context,
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
    match diff {
        Some(diff) => {
            out.push_str("\n  ],\n");
            out.push_str("  \"diff\": ");
            out.push_str(&crate::diff_json::render_diff_posture_json(diff));
            out.push_str("\n}\n");
        }
        None => out.push_str("\n  ]\n}"),
    }
    out
}

fn render_trend_fields(summary: &Summary, context: ReportContext<'_>, indent: &str) -> String {
    let signals = ReviewSignals::from_summary(summary, context);
    let mut fields = vec![
        ("review_items", signals.review_items),
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
        ("baseline_debt", signals.baseline_debt),
    ];
    if signals.policy_missing_evidence > summary.count(MatchStatus::EvidenceMissing) {
        fields.push(("policy_missing_evidence", signals.policy_missing_evidence));
    }
    if signals.broken_evidence_links > 0 {
        fields.push(("broken_evidence_links", signals.broken_evidence_links));
    }
    if signals.weak_evidence_references > 0 {
        fields.push(("weak_evidence_references", signals.weak_evidence_references));
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
