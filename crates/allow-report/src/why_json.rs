use crate::WhyReport;
use crate::contracts::WHY_ARTIFACT;
use crate::explain_json::render_explain_finding_json;
use crate::json::{
    json_string_array, option_json, push_json_fixed_artifact_preamble, render_match_outcome_json,
};
use allow_core::json_escape;

pub fn render_why_json(report: WhyReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, WHY_ARTIFACT, report.inventory);
    out.push_str("  \"finding\": ");
    // Trim leading indent spaces from explain finding helper which expects indent prefix.
    let finding_json =
        render_explain_finding_json(report.finding, report.outcome.status.as_str(), "");
    out.push_str(finding_json.trim_start());
    out.push_str(",\n");
    out.push_str("  \"outcome\": ");
    let outcome_json = render_match_outcome_json(report.outcome, "");
    out.push_str(outcome_json.trim_start());
    out.push_str(",\n");
    out.push_str("  \"candidate_entries\": [\n");
    for (index, candidate) in report.candidate_entries.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_candidate_entry_json(candidate));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"next\": {\n");
    out.push_str(&format!(
        "    \"suggested_actions\": {},\n",
        json_string_array(report.suggested_actions)
    ));
    out.push_str(&format!(
        "    \"proof_commands\": {}\n",
        json_string_array(report.proof_commands)
    ));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn render_candidate_entry_json(candidate: &crate::WhyCandidateEntry<'_>) -> String {
    format!(
        "    {{\n      \"id\": \"{}\",\n      \"kind\": \"{}\",\n      \"family\": {},\n      \"path\": {},\n      \"glob\": {},\n      \"selector_glob\": {},\n      \"mismatch_reasons\": {}\n    }}",
        json_escape(candidate.id),
        json_escape(candidate.kind),
        option_json(candidate.family),
        option_json(candidate.path),
        option_json(candidate.glob),
        option_json(candidate.selector_glob),
        json_string_array(candidate.mismatch_reasons)
    )
}
