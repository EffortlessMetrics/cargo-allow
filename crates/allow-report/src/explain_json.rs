use crate::explain_common::explain_report_status;
use crate::json::{
    json_string_array, option_json, option_u32_json, option_usize_json, push_json_artifact_preamble,
};
use crate::{
    EXPLAIN_SCHEMA_ID, EXPLAIN_SCHEMA_VERSION, EvidenceReference, ExplainReport,
    render_allow_entry_json,
};
use allow_core::{Finding, MatchOutcome, StructuralIdentity, json_escape, normalize_path};

pub fn render_explain_finding_json(finding: &Finding, status: &str, indent: &str) -> String {
    let span = finding.span.as_ref();
    format!(
        "{indent}  {{\n{indent}    \"status\": \"{}\",\n{indent}    \"kind\": \"{}\",\n{indent}    \"family\": {},\n{indent}    \"path\": \"{}\",\n{indent}    \"line\": {},\n{indent}    \"column\": {},\n{indent}    \"source_package\": {},\n{indent}    \"identity\": {},\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        json_escape(status),
        finding.kind,
        option_json(finding.family.as_deref()),
        json_escape(&normalize_path(&finding.path)),
        option_u32_json(span.map(|span| span.line)),
        option_u32_json(span.map(|span| span.column)),
        option_json(finding.source_package_name()),
        structural_identity_json(&finding.identity, indent),
        json_escape(&finding.message)
    )
}

fn structural_identity_json(identity: &StructuralIdentity, indent: &str) -> String {
    format!(
        "{{\n{indent}      \"language\": \"{}\",\n{indent}      \"crate_name\": {},\n{indent}      \"module\": {},\n{indent}      \"container\": {},\n{indent}      \"ast_kind\": \"{}\",\n{indent}      \"symbol\": {},\n{indent}      \"callee\": {},\n{indent}      \"macro_name\": {},\n{indent}      \"lint\": {},\n{indent}      \"receiver_fingerprint\": {},\n{indent}      \"target_fingerprint\": {},\n{indent}      \"normalized_snippet_hash\": {},\n{indent}      \"line_hint\": {},\n{indent}      \"column_hint\": {}\n{indent}    }}",
        json_escape(&identity.language),
        option_json(identity.crate_name.as_deref()),
        option_json(identity.module.as_deref()),
        option_json(identity.container.as_deref()),
        json_escape(&identity.ast_kind),
        option_json(identity.symbol.as_deref()),
        option_json(identity.callee.as_deref()),
        option_json(identity.macro_name.as_deref()),
        option_json(identity.lint.as_deref()),
        option_json(identity.receiver_fingerprint.as_deref()),
        option_json(identity.target_fingerprint.as_deref()),
        option_json(identity.normalized_snippet_hash.as_deref()),
        option_u32_json(identity.line_hint),
        option_u32_json(identity.column_hint)
    )
}

pub fn render_explain_json(report: ExplainReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_artifact_preamble(
        &mut out,
        EXPLAIN_SCHEMA_VERSION,
        EXPLAIN_SCHEMA_ID,
        "explain",
        report.inventory,
    );
    out.push_str("  \"allow_entry\": ");
    out.push_str(&render_allow_entry_json(report.entry, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"current_status\": \"{}\",\n    \"current_matches\": {},\n    \"match_outcomes\": {},\n    \"selector_precision\": {}\n  }},\n",
        explain_report_status(report.match_outcomes).as_str(),
        report.current_findings.len(),
        report.match_outcomes.len(),
        report.selector_precision
    ));
    out.push_str("  \"evidence_references\": [\n");
    for (index, diagnostic) in report.evidence_references.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_evidence_reference_json(diagnostic, "  "));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"current_findings\": [\n");
    for (index, finding) in report.current_findings.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        let status = report
            .match_outcomes
            .iter()
            .find(|outcome| outcome.finding_index == Some(index))
            .map(|outcome| outcome.status.as_str())
            .unwrap_or("unmatched");
        out.push_str(&render_explain_finding_json(finding, status, "  "));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"match_outcomes\": [\n");
    for (index, outcome) in report.match_outcomes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_match_outcome_json(outcome, "  "));
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

fn render_evidence_reference_json(reference: &EvidenceReference<'_>, indent: &str) -> String {
    format!(
        "{indent}  {{\n{indent}    \"raw\": \"{}\",\n{indent}    \"prefix\": {},\n{indent}    \"target\": {},\n{indent}    \"status\": \"{}\",\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        json_escape(reference.raw),
        option_json(reference.prefix),
        option_json(reference.target),
        json_escape(reference.status),
        json_escape(reference.message)
    )
}

fn render_match_outcome_json(outcome: &MatchOutcome, indent: &str) -> String {
    format!(
        "{indent}  {{\n{indent}    \"status\": \"{}\",\n{indent}    \"allow_id\": {},\n{indent}    \"finding_index\": {},\n{indent}    \"score\": {},\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        outcome.status.as_str(),
        option_json(outcome.allow_id.as_deref()),
        option_usize_json(outcome.finding_index),
        outcome.score,
        json_escape(&outcome.message)
    )
}
