use crate::EvaluationResultClass;
use crate::WhyReport;
use crate::contracts::WHY_ARTIFACT;
use crate::explain_json::render_explain_finding_json;
use crate::json::{
    json_string_array, option_json, push_json_fixed_artifact_preamble, render_match_outcome_json,
};
use allow_core::json_escape;

pub fn render_why_json(report: WhyReport<'_>) -> String {
    let result_class = report
        .evaluation
        .result_class_kind_with_scanner_completeness(report.inventory, None);
    render_why_json_with_result_class(report, result_class, None)
}

/// Render a `why` artifact with caller-supplied scanner evidence. This keeps
/// the public report shape stable while allowing scoped callers to distinguish
/// a partial repository inventory from a partial target scan.
pub fn render_why_json_with_result_class(
    report: WhyReport<'_>,
    result_class: Option<EvaluationResultClass>,
    scanner_completeness: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, WHY_ARTIFACT, report.inventory);
    out.push_str("  \"evaluation\": {\n");
    if let Some(result_class) = result_class {
        out.push_str(&format!(
            "    \"result_class\": \"{}\",\n",
            result_class.as_str()
        ));
    }
    if let Some(scanner_completeness) = scanner_completeness {
        out.push_str(&format!(
            "    \"scanner_completeness\": \"{}\",\n",
            json_escape(scanner_completeness)
        ));
    }
    out.push_str(&format!(
        "    \"scope\": \"{}\",\n    \"locality\": \"{}\",\n    \"reasons\": {}\n  }},\n",
        json_escape(report.evaluation.scope),
        json_escape(report.evaluation.locality),
        json_string_array(report.evaluation.reasons),
    ));
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
        "    \"proof_commands\": {},\n",
        json_string_array(report.proof_commands)
    ));
    out.push_str("    \"proof_plans\": [\n");
    for (index, plan) in report.proof_plans.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_proof_plan_json(plan));
    }
    out.push_str("\n    ]\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn render_proof_plan_json(plan: &crate::WhyProofPlan<'_>) -> String {
    format!(
        "      {{\n        \"program\": \"{}\",\n        \"args\": {}\n      }}",
        json_escape(plan.program),
        json_string_array(plan.args)
    )
}

fn render_candidate_entry_json(candidate: &crate::WhyCandidateEntry<'_>) -> String {
    let mut fields = vec![
        format!("      \"id\": \"{}\"", json_escape(candidate.id)),
        format!("      \"kind\": \"{}\"", json_escape(candidate.kind)),
    ];
    if let Some(family) = candidate.family {
        fields.push(format!("      \"family\": \"{}\"", json_escape(family)));
    }
    fields.extend([
        format!("      \"path\": {}", option_json(candidate.path)),
        format!("      \"glob\": {}", option_json(candidate.glob)),
        format!(
            "      \"selector_glob\": {}",
            option_json(candidate.selector_glob)
        ),
        format!(
            "      \"mismatch_reasons\": {}",
            json_string_array(candidate.mismatch_reasons)
        ),
    ]);
    format!("    {{\n{}\n    }}", fields.join(",\n"))
}
