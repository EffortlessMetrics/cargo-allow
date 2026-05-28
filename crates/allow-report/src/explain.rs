use crate::json::{
    json_string_array, option_json, option_u32_json, option_usize_json, push_json_artifact_header,
    push_json_artifact_source_context,
};
use crate::{
    CLAIM_BOUNDARY_TEXT, EXPLAIN_SCHEMA_ID, EXPLAIN_SCHEMA_VERSION, EvidenceReference,
    ExplainReport, render_allow_entry_json,
};
use allow_core::{
    AllowEntry, Finding, MatchOutcome, MatchStatus, StructuralIdentity, json_escape, normalize_path,
};

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

pub(crate) fn finding_location_text(finding: &Finding) -> String {
    match &finding.span {
        Some(span) => format!(
            "{}:{}:{}",
            normalize_path(&finding.path),
            span.line,
            span.column
        ),
        None => normalize_path(&finding.path),
    }
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
    push_json_artifact_header(
        &mut out,
        EXPLAIN_SCHEMA_VERSION,
        EXPLAIN_SCHEMA_ID,
        "explain",
    );
    push_json_artifact_source_context(&mut out, report.inventory);
    out.push_str("  \"allow_entry\": ");
    out.push_str(&render_allow_entry_json(report.entry, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"current_status\": \"{}\",\n    \"current_matches\": {},\n    \"match_outcomes\": {}\n  }},\n",
        explain_report_status(report.match_outcomes).as_str(),
        report.current_findings.len(),
        report.match_outcomes.len()
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

fn explain_report_status(outcomes: &[MatchOutcome]) -> MatchStatus {
    for status in [
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::InvalidSelector,
        MatchStatus::Ambiguous,
        MatchStatus::BaselineDebt,
        MatchStatus::Stale,
        MatchStatus::ReviewDue,
    ] {
        if outcomes.iter().any(|outcome| outcome.status == status) {
            return status;
        }
    }
    MatchStatus::Matched
}

fn explain_kind_label(entry: &AllowEntry) -> String {
    entry
        .family
        .as_ref()
        .map(|family| format!("{}.{}", entry.kind, family))
        .unwrap_or_else(|| entry.kind.to_string())
}

fn empty_as_none(value: &str) -> &str {
    if value.trim().is_empty() {
        "none"
    } else {
        value
    }
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn selector_summary(entry: &AllowEntry) -> String {
    let selector = &entry.selector;
    let mut fields = Vec::new();
    if let Some(value) = &selector.ast_kind {
        fields.push(format!("ast_kind={value}"));
    }
    if let Some(value) = &selector.container {
        fields.push(format!("container={value}"));
    }
    if let Some(value) = &selector.callee {
        fields.push(format!("callee={value}"));
    }
    if let Some(value) = &selector.macro_name {
        fields.push(format!("macro_name={value}"));
    }
    if let Some(value) = &selector.lint {
        fields.push(format!("lint={value}"));
    }
    if let Some(value) = &selector.symbol {
        fields.push(format!("symbol={value}"));
    }
    if let Some(value) = &selector.receiver_fingerprint {
        fields.push(format!("receiver={value}"));
    }
    if let Some(value) = &selector.target_fingerprint {
        fields.push(format!("target={value}"));
    }
    if let Some(value) = &selector.normalized_snippet_hash {
        fields.push(format!("normalized_snippet_hash={value}"));
    }
    if let Some(value) = selector.line_hint {
        fields.push(format!("line_hint={value}"));
    }
    if let Some(value) = &selector.glob {
        fields.push(format!("glob={value}"));
    }
    if fields.is_empty() {
        "none".to_string()
    } else {
        fields.join(", ")
    }
}

fn outcome_summary(outcomes: &[MatchOutcome]) -> String {
    let parts = [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::MissingRequiredField,
        MatchStatus::EvidenceMissing,
        MatchStatus::BaselineDebt,
    ]
    .into_iter()
    .filter_map(|status| {
        let count = outcomes
            .iter()
            .filter(|outcome| outcome.status == status)
            .count();
        (count > 0).then(|| format!("{}={count}", status.as_str()))
    })
    .collect::<Vec<_>>();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

pub fn render_explain_human(report: ExplainReport<'_>) -> String {
    let entry = report.entry;
    let mut out = String::new();
    out.push_str(&format!("{}\n", entry.id));
    out.push_str(&format!("kind: {}\n", explain_kind_label(entry)));
    out.push_str(&format!("scope: {}\n", entry.path_or_glob()));
    out.push_str(&format!("owner: {}\n", empty_as_none(&entry.owner)));
    out.push_str(&format!(
        "classification: {}\n",
        empty_as_none(&entry.classification)
    ));
    out.push_str(&format!("reason: {}\n", empty_as_none(&entry.reason)));
    out.push_str(&format!("evidence: {}\n", list_or_none(&entry.evidence)));
    if !report.evidence_references.is_empty() {
        out.push_str("\nevidence references:\n");
        for reference in report.evidence_references {
            out.push_str(&format!(
                "- {} prefix={} target={} status={} message={}\n",
                reference.raw,
                reference.prefix.unwrap_or("-"),
                reference.target.unwrap_or("-"),
                reference.status,
                reference.message
            ));
        }
    }
    if !entry.links.is_empty() {
        out.push_str(&format!("links: {}\n", entry.links.join(", ")));
    }
    if let Some(limit) = entry.occurrence_limit {
        out.push_str(&format!("occurrence_limit: {limit}\n"));
    }
    if let Some(created) = &entry.lifecycle.created {
        out.push_str(&format!("created: {created}\n"));
    }
    if let Some(expires) = &entry.lifecycle.expires {
        out.push_str(&format!("expires: {expires}\n"));
    }
    if let Some(review_after) = &entry.lifecycle.review_after {
        out.push_str(&format!("review_after: {review_after}\n"));
    }
    if let Some(last_seen) = &entry.last_seen {
        out.push_str(&format!(
            "last_seen: {}:{}\n",
            last_seen.line, last_seen.column
        ));
    }
    out.push_str(&format!("selector: {}\n\n", selector_summary(entry)));
    out.push_str(&format!(
        "current_status: {}\n",
        explain_report_status(report.match_outcomes).as_str()
    ));
    out.push_str(&format!(
        "current_matches: {}\n",
        report.current_findings.len()
    ));
    out.push_str(&format!(
        "match_outcomes: {}\n",
        outcome_summary(report.match_outcomes)
    ));
    if !report.current_findings.is_empty() {
        out.push_str("\ncurrent findings:\n");
        for (index, finding) in report.current_findings.iter().enumerate().take(20) {
            let status = report
                .match_outcomes
                .iter()
                .find(|outcome| outcome.finding_index == Some(index))
                .map(|outcome| outcome.status.as_str())
                .unwrap_or("unmatched");
            let package = finding
                .source_package_name()
                .map(|package| format!(", source_package={package}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- {status}: {} ({}{})\n",
                finding_location_text(finding),
                finding.identity.ast_kind,
                package
            ));
        }
        if report.current_findings.len() > 20 {
            out.push_str(&format!(
                "- ... {} more matching findings omitted\n",
                report.current_findings.len() - 20
            ));
        }
    }
    let attention = report
        .match_outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    if !attention.is_empty() {
        out.push_str("\nattention:\n");
        for outcome in attention.iter().take(20) {
            out.push_str(&format!(
                "- {}: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
    } else if entry.classification == "baseline_debt" {
        out.push_str("\nattention:\n");
        out.push_str(&format!(
            "- baseline_debt: {} is generated baseline_debt and still needs human review\n",
            entry.id
        ));
    }
    if !report.suggested_actions.is_empty() || !report.proof_commands.is_empty() {
        out.push_str("\nnext:\n");
        for action in report.suggested_actions.iter().take(2) {
            out.push_str(&format!("- action: {action}\n"));
        }
        for command in report.proof_commands.iter().take(3) {
            out.push_str(&format!("- proof: {command}\n"));
        }
    }
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out
}
