use crate::json::{bool_json, push_json_artifact_header, push_json_artifact_source_context};
use crate::{
    RECEIPT_SCHEMA_ID, RECEIPT_SCHEMA_VERSION, ReportContext, Summary, render_counts_fields,
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
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    push_json_artifact_header(&mut out, RECEIPT_SCHEMA_VERSION, RECEIPT_SCHEMA_ID, command);
    out.push_str(&format!(
        "  \"status\": \"{}\",\n",
        if failed { "failed" } else { "passed" }
    ));
    out.push_str(&format!("  \"failed\": {},\n", bool_json(failed)));
    push_json_artifact_source_context(&mut out, context.into());
    out.push_str("  \"counts\": {\n");
    out.push_str(&render_counts_fields(&summary, "    "));
    out.push_str("  }\n}\n");
    out
}
