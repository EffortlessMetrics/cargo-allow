use crate::contracts::EXPLAIN_ARTIFACT;
use crate::explain_common::explain_report_status;
use crate::json::{
    json_string_array, option_json, option_u32_json, push_json_fixed_artifact_preamble,
    render_match_outcome_json,
};
use crate::{EvidenceReference, ExplainReport, render_allow_entry_json};
use allow_core::{Finding, StructuralIdentity, json_escape, normalize_path};

pub fn render_explain_finding_json(finding: &Finding, status: &str, indent: &str) -> String {
    let span = finding.span.as_ref();
    let mut fields = vec![
        format!("{indent}    \"status\": \"{}\"", json_escape(status)),
        format!(
            "{indent}    \"kind\": \"{}\"",
            json_escape(&finding.kind.to_string())
        ),
    ];
    if let Some(family) = finding.family.as_deref() {
        fields.push(format!(
            "{indent}    \"family\": \"{}\"",
            json_escape(family)
        ));
    }
    fields.extend([
        format!(
            "{indent}    \"path\": \"{}\"",
            json_escape(&normalize_path(&finding.path))
        ),
        format!(
            "{indent}    \"line\": {}",
            option_u32_json(span.map(|span| span.line))
        ),
        format!(
            "{indent}    \"column\": {}",
            option_u32_json(span.map(|span| span.column))
        ),
    ]);
    if let Some(source_package) = finding.source_package_name() {
        fields.push(format!(
            "{indent}    \"source_package\": \"{}\"",
            json_escape(source_package)
        ));
    }
    fields.push(format!(
        "{indent}    \"identity\": {}",
        structural_identity_json(&finding.identity, indent)
    ));
    fields.push(format!(
        "{indent}    \"message\": \"{}\"",
        json_escape(&finding.message)
    ));
    if let Some(ledger) = finding.ledger.as_ref() {
        fields.extend([
            format!(
                "{indent}    \"ledger_id\": \"{}\"",
                json_escape(&ledger.ledger_id)
            ),
            format!(
                "{indent}    \"ledger_path\": \"{}\"",
                json_escape(&ledger.ledger_path)
            ),
            format!("{indent}    \"lane\": \"{}\"", json_escape(&ledger.lane)),
            format!("{indent}    \"mode\": \"{}\"", json_escape(&ledger.mode)),
            format!("{indent}    \"role\": \"{}\"", json_escape(&ledger.role)),
        ]);
    };
    format!("{indent}  {{\n{}\n{indent}  }}", fields.join(",\n"))
}

pub(crate) fn structural_identity_json(identity: &StructuralIdentity, indent: &str) -> String {
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
    push_json_fixed_artifact_preamble(&mut out, EXPLAIN_ARTIFACT, report.inventory);
    out.push_str("  \"allow_entry\": ");
    out.push_str(&render_allow_entry_json(report.entry, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"current_status\": \"{}\",\n    \"current_matches\": {},\n    \"match_outcomes\": {},\n    \"selector_precision\": {},\n    \"broad_scope\": {}\n  }},\n",
        explain_report_status(report.entry, report.match_outcomes).as_str(),
        report.current_findings.len(),
        report.match_outcomes.len(),
        report.selector_precision,
        crate::json::bool_json(report.broad_scope)
    ));
    out.push_str("  \"evidence_references\": [\n");
    for (index, diagnostic) in report.evidence_references.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_evidence_reference_json(diagnostic, "  "));
    }
    out.push_str("\n  ],\n");
    if !report.link_references.is_empty() {
        out.push_str("  \"link_references\": [\n");
        for (index, diagnostic) in report.link_references.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(&render_evidence_reference_json(diagnostic, "  "));
        }
        out.push_str("\n  ],\n");
    }
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
        "{indent}  {{\n{indent}    \"raw\": \"{}\",\n{indent}    \"prefix\": {},\n{indent}    \"target\": {},\n{indent}    \"status\": \"{}\",\n{indent}    \"category\": \"{}\",\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        json_escape(reference.raw),
        option_json(reference.prefix),
        option_json(reference.target),
        json_escape(reference.status),
        json_escape(reference.category),
        json_escape(reference.message)
    )
}
