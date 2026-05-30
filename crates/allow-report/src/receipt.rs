use crate::contracts::RECEIPT_ARTIFACT;
use crate::json::{bool_json, push_json_artifact_header, push_json_artifact_source_context};
use crate::{
    ARTIFACT_STATUS_FAILED, ARTIFACT_STATUS_PASSED, RECEIPT_COMMAND_CHECK, ReportContext, Summary,
    baseline_debt_count, render_count_fields_with_policy_context,
};
use allow_core::MatchOutcome;

pub fn render_receipt(command: &str, outcomes: &[MatchOutcome], failed: bool) -> String {
    render_receipt_with_context(command, outcomes, failed, ReportContext::default())
}

pub fn render_receipt_with_context(
    command: &str,
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
    out.push_str(&format!(
        "  \"status\": \"{}\",\n",
        if failed {
            ARTIFACT_STATUS_FAILED
        } else {
            ARTIFACT_STATUS_PASSED
        }
    ));
    out.push_str(&format!("  \"failed\": {},\n", bool_json(failed)));
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
    out.push_str("  }\n}\n");
    out
}
