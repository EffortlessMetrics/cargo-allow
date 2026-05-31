use crate::explain_common::{explain_report_status, finding_location_text};
use crate::{CLAIM_BOUNDARY_TEXT, EvidenceReference, ExplainReport};
use allow_core::{AllowEntry, MatchOutcome, MatchStatus};

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
        out.push_str("\nevidence diagnostics:\n");
        for reference in report.evidence_references {
            out.push_str(&format!("- {}\n", evidence_reference_summary(reference)));
            out.push_str(&format!("  message: {}\n", reference.message));
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
    out.push_str(&format!("selector: {}\n", selector_summary(entry)));
    out.push_str(&format!(
        "selector_precision: {}\n",
        report.selector_precision
    ));
    out.push_str(&format!("broad_scope: {}\n\n", report.broad_scope));
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
        for command in report.proof_commands.iter().take(5) {
            out.push_str(&format!("- proof: {command}\n"));
        }
    }
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out
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

fn evidence_reference_summary(reference: &EvidenceReference<'_>) -> String {
    format!(
        "{}: {} (prefix={}, target={})",
        reference.status,
        reference.raw,
        reference.prefix.unwrap_or("-"),
        reference.target.unwrap_or("-")
    )
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
