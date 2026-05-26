use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape, normalize_path};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub by_status: BTreeMap<MatchStatus, usize>,
}

impl Summary {
    pub fn from_outcomes(outcomes: &[MatchOutcome]) -> Self {
        let mut summary = Self {
            total: outcomes.len(),
            by_status: BTreeMap::new(),
        };
        for outcome in outcomes {
            *summary.by_status.entry(outcome.status).or_insert(0) += 1;
        }
        summary
    }
    pub fn count(&self, status: MatchStatus) -> usize {
        *self.by_status.get(&status).unwrap_or(&0)
    }
}

pub fn render_human(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str(&format!("cargo-allow {command}\n\n"));
    out.push_str(&format!("Findings scanned: {}\n", findings.len()));
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
    ] {
        let count = summary.count(status);
        if count > 0 {
            out.push_str(&format!("  {:24} {}\n", status.as_str(), count));
        }
    }
    if outcomes.is_empty() {
        out.push_str("  no outcomes\n");
    }
    out.push('\n');
    for outcome in outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .take(80)
    {
        out.push_str(&format!(
            "{}: {}\n",
            outcome.status.as_str(),
            outcome.message
        ));
    }
    out.push_str("\nClaim boundary: source syntax only; macro expansion and type information were not analyzed.\n");
    out.push_str(if failed {
        "Result: failed\n"
    } else {
        "Result: passed/advisory\n"
    });
    out
}

pub fn render_markdown(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str(&format!("# cargo-allow {command}\n\n"));
    out.push_str(&format!(
        "**Result:** {}\n\n",
        if failed { "failed" } else { "passed/advisory" }
    ));
    out.push_str(&format!("Findings scanned: `{}`\n\n", findings.len()));
    out.push_str("| Status | Count |\n|---|---:|\n");
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
    ] {
        let count = summary.count(status);
        out.push_str(&format!("| `{}` | {} |\n", status.as_str(), count));
    }
    out.push_str("\n## Non-matched outcomes\n\n");
    for outcome in outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .take(100)
    {
        out.push_str(&format!(
            "- `{}`: {}\n",
            outcome.status.as_str(),
            outcome.message
        ));
    }
    out.push_str("\n> Claim boundary: source syntax only; macro expansion and type information were not analyzed.\n");
    out
}

pub fn render_json(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema_version\": 1,\n");
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str(&format!("  \"command\": \"{}\",\n", json_escape(command)));
    out.push_str(&format!(
        "  \"status\": \"{}\",\n",
        if failed { "failed" } else { "passed" }
    ));
    out.push_str("  \"claim_boundary\": [\"source_syntax_only\", \"macro_expansion_not_analyzed\", \"type_information_not_analyzed\"],\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"findings\": {},\n", findings.len()));
    out.push_str(&format!(
        "    \"matched\": {},\n",
        summary.count(MatchStatus::Matched)
    ));
    out.push_str(&format!(
        "    \"new\": {},\n",
        summary.count(MatchStatus::New)
    ));
    out.push_str(&format!(
        "    \"expired\": {},\n",
        summary.count(MatchStatus::Expired)
    ));
    out.push_str(&format!(
        "    \"stale\": {},\n",
        summary.count(MatchStatus::Stale)
    ));
    out.push_str(&format!(
        "    \"ambiguous\": {},\n",
        summary.count(MatchStatus::Ambiguous)
    ));
    out.push_str(&format!(
        "    \"evidence_missing\": {}\n",
        summary.count(MatchStatus::EvidenceMissing)
    ));
    out.push_str("  },\n");
    out.push_str("  \"outcomes\": [\n");
    for (i, outcome) in outcomes.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {");
        out.push_str(&format!("\"status\": \"{}\", ", outcome.status.as_str()));
        out.push_str(&format!(
            "\"allow_id\": {}, ",
            option_json(outcome.allow_id.as_deref())
        ));
        out.push_str(&format!(
            "\"finding_index\": {}, ",
            outcome
                .finding_index
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
        out.push_str(&format!("\"score\": {}, ", outcome.score));
        out.push_str(&format!(
            "\"message\": \"{}\"",
            json_escape(&outcome.message)
        ));
        out.push('}');
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
            "\"ast_kind\": \"{}\"",
            json_escape(&finding.identity.ast_kind)
        ));
        out.push('}');
    }
    out.push_str("\n  ]\n}");
    out
}

pub fn render_receipt(command: &str, outcomes: &[MatchOutcome], failed: bool) -> String {
    let summary = Summary::from_outcomes(outcomes);
    format!(
        "{{\n  \"schema_version\": 1,\n  \"tool\": \"cargo-allow\",\n  \"command\": \"{}\",\n  \"status\": \"{}\",\n  \"counts\": {{\n    \"matched\": {},\n    \"new\": {},\n    \"expired\": {},\n    \"stale\": {},\n    \"ambiguous\": {}\n  }}\n}}\n",
        json_escape(command),
        if failed { "failed" } else { "passed" },
        summary.count(MatchStatus::Matched),
        summary.count(MatchStatus::New),
        summary.count(MatchStatus::Expired),
        summary.count(MatchStatus::Stale),
        summary.count(MatchStatus::Ambiguous)
    )
}

fn option_json(value: Option<&str>) -> String {
    value
        .map(|v| format!("\"{}\"", json_escape(v)))
        .unwrap_or_else(|| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_contains_claim_boundary() {
        let json = render_json("audit", &[], &[], false);
        assert!(json.contains("macro_expansion_not_analyzed"));
    }
}
