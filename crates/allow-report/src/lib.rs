use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, json_escape, normalize_path};
use std::collections::BTreeMap;

pub const REPORT_SCHEMA_VERSION: u32 = 1;
pub const REPORT_SCHEMA_ID: &str = "cargo-allow.report.v1";
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const RECEIPT_SCHEMA_ID: &str = "cargo-allow.receipt.v1";
pub const WORKLIST_SCHEMA_VERSION: u32 = 1;
pub const WORKLIST_SCHEMA_ID: &str = "cargo-allow.worklist.v1";
pub const LIST_SCHEMA_VERSION: u32 = 1;
pub const LIST_SCHEMA_ID: &str = "cargo-allow.list.v1";
pub const EXPLAIN_SCHEMA_VERSION: u32 = 1;
pub const EXPLAIN_SCHEMA_ID: &str = "cargo-allow.explain.v1";
pub const PRUNE_SCHEMA_VERSION: u32 = 1;
pub const PRUNE_SCHEMA_ID: &str = "cargo-allow.prune.v1";
pub const DOCTOR_SCHEMA_VERSION: u32 = 1;
pub const DOCTOR_SCHEMA_ID: &str = "cargo-allow.doctor.v1";
pub const PROPOSE_SCHEMA_VERSION: u32 = 1;
pub const PROPOSE_SCHEMA_ID: &str = "cargo-allow.propose.v1";

pub const CLAIM_BOUNDARY: &[&str] = &[
    "source_tree_inventory",
    "source_syntax_only",
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "repository_code_not_executed",
];

pub const SCANNER_LIMITATIONS: &[&str] = &[
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "repository_code_not_executed",
];

pub const CLAIM_BOUNDARY_TEXT: &str = "Claim boundary: scanned source-tree/source syntax only; cargo-allow did not invoke Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros, or repository code. Macro expansion, macro token-tree contents, type information, MIR, build output, control flow, and data flow were not analyzed.";

#[derive(Debug, Clone, Copy)]
pub struct ReportContext<'a> {
    pub inventory_source: &'a str,
    pub source_tree_root: Option<&'a str>,
    pub inventory_files: Option<usize>,
    pub baseline_debt_entries: Option<usize>,
}

impl Default for ReportContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            baseline_debt_entries: None,
        }
    }
}

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
    render_human_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_human_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str(&format!("cargo-allow {command}\n\n"));
    out.push_str(&format!("Findings scanned: {}\n", findings.len()));
    out.push_str(&format!(
        "Inventory: source_tree/source_syntax via {}{}\n",
        context.inventory_source,
        inventory_files_suffix(context)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
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
    render_non_rust_human(findings, outcomes, &mut out);
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
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
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
    render_markdown_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_markdown_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str(&format!("# cargo-allow {command}\n\n"));
    out.push_str(&format!(
        "**Result:** {}\n\n",
        if failed { "failed" } else { "passed/advisory" }
    ));
    out.push_str(&format!("Findings scanned: `{}`\n\n", findings.len()));
    out.push_str(&format!(
        "Inventory: `source_tree` / `source_syntax` via `{}`{}\n\n",
        json_escape(context.inventory_source),
        inventory_files_markdown_suffix(context)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(
            "Source tree root: `{}`\n\n",
            markdown_inline_code(root)
        ));
    }
    out.push_str("| Status | Count |\n|---|---:|\n");
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
    ] {
        let count = summary.count(status);
        out.push_str(&format!("| `{}` | {} |\n", status.as_str(), count));
    }
    if command == "audit" {
        render_audit_summary_markdown(&summary, outcomes, context, &mut out);
    }
    render_non_rust_markdown(findings, outcomes, &mut out);
    let non_matched = outcomes
        .iter()
        .filter(|o| o.status != MatchStatus::Matched)
        .take(100)
        .collect::<Vec<_>>();
    if !non_matched.is_empty() {
        out.push_str("\n## Non-matched outcomes\n\n");
        for outcome in non_matched {
            out.push_str(&format!(
                "- `{}`: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
    }
    out.push_str("\n> ");
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

pub fn render_html(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_html_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_html_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"utf-8\">\n");
    out.push_str(&format!(
        "  <title>cargo-allow {}</title>\n",
        html_escape(command)
    ));
    out.push_str("  <style>body{font-family:system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;line-height:1.45}table{border-collapse:collapse;width:100%;margin:1rem 0}th,td{border:1px solid #d0d7de;padding:.4rem .55rem;text-align:left}th{background:#f6f8fa}td.count{text-align:right;font-variant-numeric:tabular-nums}.status{font-weight:700}.failed{color:#b42318}.passed{color:#1a7f37}code{background:#f6f8fa;padding:.1rem .25rem;border-radius:4px}.claim{border-left:4px solid #57606a;padding-left:1rem;color:#57606a}</style>\n");
    out.push_str("</head>\n<body>\n");
    out.push_str(&format!("<h1>cargo-allow {}</h1>\n", html_escape(command)));
    out.push_str(&format!(
        "<p class=\"status {}\">Result: {}</p>\n",
        if failed { "failed" } else { "passed" },
        if failed { "failed" } else { "passed/advisory" }
    ));
    out.push_str(&format!(
        "<p>Findings scanned: <code>{}</code></p>\n",
        findings.len()
    ));
    out.push_str(&format!(
        "<p>Inventory: <code>source_tree</code> / <code>source_syntax</code> via <code>{}</code>{}</p>\n",
        html_escape(context.inventory_source),
        inventory_files_html_suffix(context)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(
            "<p>Source tree root: <code>{}</code></p>\n",
            html_escape(root)
        ));
    }
    out.push_str("<h2>Status Counts</h2>\n");
    render_status_count_table_html(&summary, &mut out);
    if command == "audit" {
        render_audit_summary_html(&summary, outcomes, context, &mut out);
    }
    render_non_rust_html(findings, outcomes, &mut out);
    render_non_matched_html(outcomes, &mut out);
    out.push_str("<h2>Claim Boundary</h2>\n");
    out.push_str(&format!(
        "<p class=\"claim\">{}</p>\n",
        html_escape(CLAIM_BOUNDARY_TEXT)
    ));
    out.push_str("</body>\n</html>\n");
    out
}

fn render_status_count_table_html(summary: &Summary, out: &mut String) {
    out.push_str("<table><thead><tr><th>Status</th><th>Count</th></tr></thead><tbody>\n");
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
    ] {
        out.push_str(&format!(
            "<tr><td><code>{}</code></td><td class=\"count\">{}</td></tr>\n",
            status.as_str(),
            summary.count(status)
        ));
    }
    out.push_str("</tbody></table>\n");
}

fn render_audit_summary_html(
    summary: &Summary,
    outcomes: &[MatchOutcome],
    context: ReportContext<'_>,
    out: &mut String,
) {
    let baseline_debt = baseline_debt_count(summary, context);
    let review_items = review_item_count_with_baseline(summary, baseline_debt);
    let queue = audit_review_queue(outcomes);
    out.push_str("<h2>Audit Summary</h2>\n");
    out.push_str("<table><thead><tr><th>Signal</th><th>Count</th></tr></thead><tbody>\n");
    for (name, value) in [
        ("Match outcomes", summary.total),
        ("Review items", review_items),
        ("New unreceipted", summary.count(MatchStatus::New)),
        ("Expired", summary.count(MatchStatus::Expired)),
        ("Evidence gaps", summary.count(MatchStatus::EvidenceMissing)),
        ("Baseline debt", baseline_debt),
    ] {
        out.push_str(&format!(
            "<tr><td>{}</td><td class=\"count\">{}</td></tr>\n",
            html_escape(name),
            value
        ));
    }
    out.push_str("</tbody></table>\n");
    if review_items == 0 {
        out.push_str("<p>Recommended next step: keep <code>cargo-allow check --mode no-new</code> in CI.</p>\n");
    } else if queue.is_empty() && baseline_debt > 0 {
        out.push_str("<p>Recommended next step: run <code>cargo-allow worklist --format json</code> to review generated baseline debt.</p>\n");
    } else {
        out.push_str(
            "<p>Recommended next step: review the queue below before tightening policy.</p>\n",
        );
    }
    if !queue.is_empty() {
        out.push_str("<h2>Audit Review Queue</h2>\n<ul>\n");
        for outcome in queue {
            out.push_str(&format!(
                "<li><code>{}</code>: {}</li>\n",
                outcome.status.as_str(),
                html_escape(&outcome.message)
            ));
        }
        out.push_str("</ul>\n");
    }
}

fn render_audit_summary_markdown(
    summary: &Summary,
    outcomes: &[MatchOutcome],
    context: ReportContext<'_>,
    out: &mut String,
) {
    let review_statuses = [
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::BaselineDebt,
        MatchStatus::Stale,
        MatchStatus::ReviewDue,
    ];
    let baseline_debt = baseline_debt_count(summary, context);
    let review_items = review_item_count_with_baseline(summary, baseline_debt);
    let queue = outcomes
        .iter()
        .filter(|outcome| review_statuses.contains(&outcome.status))
        .take(20)
        .collect::<Vec<_>>();
    out.push_str("\n## Audit Summary\n\n");
    out.push_str("| Signal | Count |\n|---|---:|\n");
    out.push_str(&format!("| Match outcomes | {} |\n", summary.total));
    out.push_str(&format!("| Review items | {} |\n", review_items));
    out.push_str(&format!(
        "| New unreceipted | {} |\n",
        summary.count(MatchStatus::New)
    ));
    out.push_str(&format!(
        "| Expired | {} |\n",
        summary.count(MatchStatus::Expired)
    ));
    out.push_str(&format!(
        "| Evidence gaps | {} |\n",
        summary.count(MatchStatus::EvidenceMissing)
    ));
    out.push_str(&format!("| Baseline debt | {} |\n", baseline_debt));
    if review_items == 0 {
        out.push_str("\nRecommended next step: keep `cargo-allow check --mode no-new` in CI.\n");
    } else if queue.is_empty() && baseline_debt > 0 {
        out.push_str("\nRecommended next step: run `cargo-allow worklist --format json` to review generated baseline debt.\n");
    } else {
        out.push_str("\nRecommended next step: review the queue below before tightening policy.\n");
    }

    if !queue.is_empty() {
        out.push_str("\n## Audit Review Queue\n\n");
        for outcome in queue {
            out.push_str(&format!(
                "- `{}`: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
    }
}

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
    let summary = Summary::from_outcomes(outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema_version\": {REPORT_SCHEMA_VERSION},\n"));
    out.push_str(&format!("  \"schema_id\": \"{REPORT_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str(&format!("  \"command\": \"{}\",\n", json_escape(command)));
    out.push_str(&format!(
        "  \"status\": \"{}\",\n",
        if failed { "failed" } else { "passed" }
    ));
    out.push_str(&format!("  \"failed\": {},\n", bool_json(failed)));
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"findings\": {},\n", findings.len()));
    out.push_str(&format!("    \"outcomes\": {},\n", summary.total));
    out.push_str(&render_counts_fields(&summary, "    "));
    out.push_str("  },\n");
    out.push_str("  \"trend\": {\n");
    out.push_str(&render_trend_fields(&summary, context, "    "));
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
            "\"source_package\": {}, ",
            option_json(finding.identity.crate_name.as_deref())
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

pub fn render_sarif(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
) -> String {
    render_sarif_with_context(
        command,
        findings,
        outcomes,
        failed,
        ReportContext::default(),
    )
}

pub fn render_sarif_with_context(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    let reportable = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"$schema\": \"https://json.schemastore.org/sarif-2.1.0.json\",\n");
    out.push_str("  \"version\": \"2.1.0\",\n");
    out.push_str("  \"runs\": [\n");
    out.push_str("    {\n");
    out.push_str("      \"tool\": {\n");
    out.push_str("        \"driver\": {\n");
    out.push_str("          \"name\": \"cargo-allow\",\n");
    out.push_str(
        "          \"informationUri\": \"https://github.com/EffortlessMetrics/cargo-allow\",\n",
    );
    out.push_str("          \"rules\": [\n");
    for (index, status) in SARIF_STATUSES.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_sarif_rule(*status));
    }
    out.push_str("\n          ]\n");
    out.push_str("        }\n");
    out.push_str("      },\n");
    out.push_str("      \"properties\": {\n");
    out.push_str(&format!(
        "        \"command\": \"{}\",\n",
        json_escape(command)
    ));
    out.push_str(&format!(
        "        \"status\": \"{}\",\n",
        if failed { "failed" } else { "passed" }
    ));
    out.push_str(&format!("        \"failed\": {},\n", bool_json(failed)));
    out.push_str("        \"inventory\": ");
    out.push_str(&inventory_json(context, "        "));
    out.push_str(",\n");
    out.push_str(&format!(
        "        \"claim_boundary\": {},\n",
        json_string_array(CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "        \"scanner_limitations\": {}\n",
        json_string_array(SCANNER_LIMITATIONS)
    ));
    out.push_str("      },\n");
    out.push_str("      \"results\": [\n");
    for (index, outcome) in reportable.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
        out.push_str(&render_sarif_result(outcome, finding));
    }
    out.push_str("\n      ]\n");
    out.push_str("    }\n");
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

const SARIF_STATUSES: &[MatchStatus] = &[
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::ReviewDue,
    MatchStatus::Stale,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::MissingRequiredField,
    MatchStatus::EvidenceMissing,
    MatchStatus::BaselineDebt,
];

fn render_sarif_rule(status: MatchStatus) -> String {
    format!(
        "            {{\"id\": \"{}\", \"name\": \"{}\", \"shortDescription\": {{\"text\": \"{}\"}}}}",
        sarif_rule_id(status),
        status.as_str(),
        sarif_rule_description(status)
    )
}

fn render_sarif_result(outcome: &MatchOutcome, finding: Option<&Finding>) -> String {
    let mut out = String::new();
    out.push_str("        {\n");
    out.push_str(&format!(
        "          \"ruleId\": \"{}\",\n",
        sarif_rule_id(outcome.status)
    ));
    out.push_str(&format!(
        "          \"level\": \"{}\",\n",
        sarif_level(outcome.status)
    ));
    out.push_str(&format!(
        "          \"message\": {{\"text\": \"{}\"}},\n",
        json_escape(&outcome.message)
    ));
    out.push_str("          \"properties\": {\n");
    out.push_str(&format!(
        "            \"status\": \"{}\",\n",
        outcome.status.as_str()
    ));
    out.push_str(&format!(
        "            \"allow_id\": {},\n",
        option_json(outcome.allow_id.as_deref())
    ));
    out.push_str(&format!(
        "            \"finding_index\": {},\n",
        outcome
            .finding_index
            .map(|idx| idx.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!("            \"score\": {},\n", outcome.score));
    out.push_str(&format!(
        "            \"source_package\": {}\n",
        option_json(finding.and_then(|finding| finding.identity.crate_name.as_deref()))
    ));
    out.push_str("          }");
    if let Some(finding) = finding {
        out.push_str(",\n");
        out.push_str("          \"locations\": [\n");
        out.push_str(&render_sarif_location(finding));
        out.push_str("\n          ]\n");
        out.push_str("        }");
    } else {
        out.push('\n');
        out.push_str("        }");
    }
    out
}

fn render_sarif_location(finding: &Finding) -> String {
    let mut out = String::new();
    out.push_str("            {\n");
    out.push_str("              \"physicalLocation\": {\n");
    out.push_str(&format!(
        "                \"artifactLocation\": {{\"uri\": \"{}\"}}",
        json_escape(&normalize_path(&finding.path))
    ));
    if let Some(span) = &finding.span {
        out.push_str(",\n");
        out.push_str("                \"region\": {\n");
        out.push_str(&format!(
            "                  \"startLine\": {},\n",
            span.line
        ));
        out.push_str(&format!(
            "                  \"startColumn\": {}\n",
            span.column
        ));
        out.push_str("                }\n");
        out.push_str("              }\n");
    } else {
        out.push('\n');
        out.push_str("              }\n");
    }
    out.push_str("            }");
    out
}

fn sarif_rule_id(status: MatchStatus) -> String {
    format!("cargo-allow/{}", status.as_str())
}

fn sarif_rule_description(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::New => "New unreceipted source-tree exception finding.",
        MatchStatus::Expired => "Matched allow entry is expired.",
        MatchStatus::ReviewDue => "Matched allow entry is due for review.",
        MatchStatus::Stale => "Allow entry did not match any current finding.",
        MatchStatus::Ambiguous => "Selector matched ambiguously and needs narrowing.",
        MatchStatus::InvalidSelector => "Allow entry selector is invalid.",
        MatchStatus::MissingRequiredField => "Allow entry is missing required policy metadata.",
        MatchStatus::EvidenceMissing => "Allow entry is missing required evidence.",
        MatchStatus::BaselineDebt => "Generated baseline debt remains in policy.",
        MatchStatus::Matched => "Finding matched policy.",
    }
}

fn sarif_level(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::New
        | MatchStatus::Expired
        | MatchStatus::Ambiguous
        | MatchStatus::InvalidSelector
        | MatchStatus::MissingRequiredField
        | MatchStatus::EvidenceMissing => "error",
        MatchStatus::ReviewDue | MatchStatus::BaselineDebt => "warning",
        MatchStatus::Stale => "note",
        MatchStatus::Matched => "none",
    }
}

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
    format!(
        "{{\n  \"schema_version\": {RECEIPT_SCHEMA_VERSION},\n  \"schema_id\": \"{RECEIPT_SCHEMA_ID}\",\n  \"tool\": \"cargo-allow\",\n  \"command\": \"{}\",\n  \"status\": \"{}\",\n  \"failed\": {},\n  \"claim_boundary\": {},\n  \"scanner_limitations\": {},\n  \"inventory\": {},\n  \"counts\": {{\n{}  }}\n}}\n",
        json_escape(command),
        if failed { "failed" } else { "passed" },
        bool_json(failed),
        json_string_array(CLAIM_BOUNDARY),
        json_string_array(SCANNER_LIMITATIONS),
        inventory_json(context, "  "),
        render_counts_fields(&summary, "    ")
    )
}

fn option_json(value: Option<&str>) -> String {
    value
        .map(|v| format!("\"{}\"", json_escape(v)))
        .unwrap_or_else(|| "null".to_string())
}

fn bool_json(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn json_string_array(values: &[&str]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn inventory_json(context: ReportContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"files_scanned\": {files}"));
    }
    out.push('\n');
    out.push_str(&format!("{indent}}}"));
    out
}

fn inventory_files_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory_files
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
}

fn inventory_files_markdown_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory_files
        .map(|files| format!("; files scanned: `{files}`"))
        .unwrap_or_default()
}

fn inventory_files_html_suffix(context: ReportContext<'_>) -> String {
    context
        .inventory_files
        .map(|files| format!("; files scanned: <code>{files}</code>"))
        .unwrap_or_default()
}

fn markdown_inline_code(value: &str) -> String {
    json_escape(value).replace('`', "\\`")
}

fn render_counts_fields(summary: &Summary, indent: &str) -> String {
    let statuses = [
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
    ];
    statuses
        .iter()
        .enumerate()
        .map(|(idx, status)| {
            let comma = if idx + 1 == statuses.len() { "" } else { "," };
            format!(
                "{indent}\"{}\": {}{comma}\n",
                status.as_str(),
                summary.count(*status)
            )
        })
        .collect::<String>()
}

fn render_trend_fields(summary: &Summary, context: ReportContext<'_>, indent: &str) -> String {
    let baseline_debt = baseline_debt_count(summary, context);
    let fields = [
        (
            "review_items",
            review_item_count_with_baseline(summary, baseline_debt),
        ),
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
        ("baseline_debt", baseline_debt),
    ];
    fields
        .iter()
        .enumerate()
        .map(|(idx, (name, value))| {
            let comma = if idx + 1 == fields.len() { "" } else { "," };
            format!("{indent}\"{name}\": {value}{comma}\n")
        })
        .collect()
}

fn review_item_count_with_baseline(summary: &Summary, baseline_debt: usize) -> usize {
    [
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::MissingRequiredField,
        MatchStatus::EvidenceMissing,
    ]
    .iter()
    .map(|status| summary.count(*status))
    .sum::<usize>()
        + baseline_debt
}

fn baseline_debt_count(summary: &Summary, context: ReportContext<'_>) -> usize {
    context
        .baseline_debt_entries
        .unwrap_or_else(|| summary.count(MatchStatus::BaselineDebt))
}

fn audit_review_queue(outcomes: &[MatchOutcome]) -> Vec<&MatchOutcome> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .take(20)
        .collect()
}

#[derive(Debug, Default)]
struct FilePosture {
    total: usize,
    by_family: BTreeMap<String, usize>,
    matched: usize,
    new: usize,
    generated: usize,
}

impl FilePosture {
    fn from_report(findings: &[Finding], outcomes: &[MatchOutcome]) -> Self {
        let mut posture = Self::default();
        for finding in findings.iter().filter(|finding| is_file_finding(finding)) {
            posture.total += 1;
            if finding.kind == FindingKind::GeneratedCode {
                posture.generated += 1;
            }
            *posture
                .by_family
                .entry(
                    finding
                        .family
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_insert(0) += 1;
        }
        for outcome in outcomes {
            let applies_to_file = outcome
                .finding_index
                .and_then(|idx| findings.get(idx))
                .map(is_file_finding)
                .unwrap_or(false);
            match outcome.status {
                MatchStatus::Matched if applies_to_file => posture.matched += 1,
                MatchStatus::New if applies_to_file => posture.new += 1,
                _ => {}
            }
        }
        posture
    }

    fn has_files(&self) -> bool {
        self.total > 0
    }
}

fn render_non_rust_human(findings: &[Finding], outcomes: &[MatchOutcome], out: &mut String) {
    let posture = FilePosture::from_report(findings, outcomes);
    if !posture.has_files() {
        return;
    }
    out.push('\n');
    out.push_str("Non-Rust file inventory:\n");
    out.push_str(&format!("  files scanned              {}\n", posture.total));
    out.push_str(&format!(
        "  matched                    {}\n",
        posture.matched
    ));
    out.push_str(&format!("  new                        {}\n", posture.new));
    out.push_str(&format!(
        "  generated                  {}\n",
        posture.generated
    ));
    if !posture.by_family.is_empty() {
        out.push_str("  by family:\n");
        for (family, count) in posture.by_family {
            out.push_str(&format!("    {:24} {}\n", family, count));
        }
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        out.push_str("  files:\n");
        for row in rows.into_iter().take(40) {
            out.push_str(&format!(
                "    {:12} {:24} {}\n",
                row.status, row.family, row.path
            ));
        }
    }
}

fn render_non_rust_markdown(findings: &[Finding], outcomes: &[MatchOutcome], out: &mut String) {
    let posture = FilePosture::from_report(findings, outcomes);
    if !posture.has_files() {
        return;
    }
    out.push_str("\n## Non-Rust File Inventory\n\n");
    out.push_str("| Metric | Count |\n|---|---:|\n");
    out.push_str(&format!("| Files scanned | {} |\n", posture.total));
    out.push_str(&format!("| Matched | {} |\n", posture.matched));
    out.push_str(&format!("| New | {} |\n", posture.new));
    out.push_str(&format!("| Generated | {} |\n", posture.generated));
    if !posture.by_family.is_empty() {
        out.push_str("\n| Family | Count |\n|---|---:|\n");
        for (family, count) in posture.by_family {
            out.push_str(&format!("| `{}` | {} |\n", markdown_cell(&family), count));
        }
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        out.push_str("\n| Status | Family | Path |\n|---|---|---|\n");
        for row in rows.into_iter().take(60) {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` |\n",
                markdown_cell(row.status),
                markdown_cell(&row.family),
                markdown_cell(&row.path)
            ));
        }
    }
}

fn render_non_rust_html(findings: &[Finding], outcomes: &[MatchOutcome], out: &mut String) {
    let posture = FilePosture::from_report(findings, outcomes);
    if !posture.has_files() {
        return;
    }
    out.push_str("<h2>Non-Rust File Inventory</h2>\n");
    out.push_str("<table><thead><tr><th>Metric</th><th>Count</th></tr></thead><tbody>\n");
    for (name, value) in [
        ("Files scanned", posture.total),
        ("Matched", posture.matched),
        ("New", posture.new),
        ("Generated", posture.generated),
    ] {
        out.push_str(&format!(
            "<tr><td>{}</td><td class=\"count\">{}</td></tr>\n",
            html_escape(name),
            value
        ));
    }
    out.push_str("</tbody></table>\n");
    if !posture.by_family.is_empty() {
        out.push_str("<table><thead><tr><th>Family</th><th>Count</th></tr></thead><tbody>\n");
        for (family, count) in posture.by_family {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td class=\"count\">{}</td></tr>\n",
                html_escape(&family),
                count
            ));
        }
        out.push_str("</tbody></table>\n");
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        out.push_str(
            "<table><thead><tr><th>Status</th><th>Family</th><th>Path</th></tr></thead><tbody>\n",
        );
        for row in rows.into_iter().take(60) {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td><td><code>{}</code></td></tr>\n",
                html_escape(row.status),
                html_escape(&row.family),
                html_escape(&row.path)
            ));
        }
        out.push_str("</tbody></table>\n");
    }
}

fn render_non_matched_html(outcomes: &[MatchOutcome], out: &mut String) {
    let non_matched = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .take(100)
        .collect::<Vec<_>>();
    if non_matched.is_empty() {
        return;
    }
    out.push_str("<h2>Non-matched Outcomes</h2>\n<ul>\n");
    for outcome in non_matched {
        out.push_str(&format!(
            "<li><code>{}</code>: {}</li>\n",
            outcome.status.as_str(),
            html_escape(&outcome.message)
        ));
    }
    out.push_str("</ul>\n");
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('`', "\\`")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn is_file_finding(finding: &Finding) -> bool {
    matches!(
        finding.kind,
        FindingKind::NonRustFile | FindingKind::GeneratedCode
    )
}

#[derive(Debug)]
struct FileRow {
    status: &'static str,
    family: String,
    path: String,
}

fn non_rust_file_rows(findings: &[Finding], outcomes: &[MatchOutcome]) -> Vec<FileRow> {
    let mut status_by_index = BTreeMap::new();
    for outcome in outcomes {
        if let Some(index) = outcome.finding_index {
            status_by_index.insert(index, outcome.status.as_str());
        }
    }
    let mut rows = findings
        .iter()
        .enumerate()
        .filter(|(_, finding)| is_file_finding(finding))
        .map(|(index, finding)| FileRow {
            status: status_by_index.get(&index).copied().unwrap_or("unmatched"),
            family: finding
                .family
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            path: normalize_path(&finding.path),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.family.cmp(&right.family))
            .then_with(|| left.status.cmp(right.status))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Finding, FindingKind, Span, StructuralIdentity};
    use std::path::PathBuf;

    fn context(source: &'static str) -> ReportContext<'static> {
        ReportContext {
            inventory_source: source,
            ..ReportContext::default()
        }
    }

    #[test]
    fn json_contains_claim_boundary() {
        let json = render_json_with_context(
            "audit",
            &[],
            &[],
            false,
            ReportContext {
                inventory_source: "filesystem_fallback",
                source_tree_root: Some("fixtures/source-snapshot"),
                inventory_files: Some(7),
                ..ReportContext::default()
            },
        );
        assert!(CLAIM_BOUNDARY.contains(&"source_tree_inventory"));
        assert!(SCANNER_LIMITATIONS.contains(&"cargo_metadata_not_invoked"));
        assert_eq!(CLAIM_BOUNDARY.len(), SCANNER_LIMITATIONS.len() + 2);
        assert!(json.contains("source_tree_inventory"));
        assert!(json.contains("cargo_metadata_not_invoked"));
        assert!(json.contains("cargo_commands_not_invoked"));
        assert!(json.contains("rustc_not_invoked"));
        assert!(json.contains("clippy_not_invoked"));
        assert!(json.contains("build_scripts_not_executed"));
        assert!(json.contains("proc_macros_not_executed"));
        assert!(json.contains("macro_expansion_not_analyzed"));
        assert!(json.contains("macro_token_tree_contents_not_analyzed"));
        assert!(json.contains("repository_code_not_executed"));
    }

    #[test]
    fn json_report_exposes_v1_schema_contract() {
        let json = render_json_with_context(
            "audit",
            &[],
            &[],
            false,
            ReportContext {
                inventory_source: "filesystem_fallback",
                source_tree_root: Some("fixtures/source-snapshot"),
                inventory_files: Some(7),
                ..ReportContext::default()
            },
        );
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"schema_id\": \"cargo-allow.report.v1\""));
        assert!(json.contains("\"failed\": false"));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"scope\": \"source_tree\""));
        assert!(json.contains("\"scanner\": \"source_syntax\""));
        assert!(json.contains("\"source\": \"filesystem_fallback\""));
        assert!(json.contains("\"root\": \"fixtures/source-snapshot\""));
        assert!(json.contains("\"files_scanned\": 7"));
        assert!(json.contains("\"review_due\": 0"));
        assert!(json.contains("\"baseline_debt\": 0"));
        assert!(json.contains("\"trend\""));
        assert!(json.contains("\"review_items\": 0"));
    }

    #[test]
    fn json_report_exposes_trend_metrics() {
        let outcomes = vec![
            outcome(MatchStatus::New, Some(0)),
            outcome(MatchStatus::EvidenceMissing, Some(1)),
            outcome(MatchStatus::Stale, None),
        ];

        let json = render_json("audit", &[], &outcomes, false);

        assert!(json.contains("\"trend\""));
        assert!(json.contains("\"review_items\": 3"));
        assert!(json.contains("\"new\": 1"));
        assert!(json.contains("\"stale\": 1"));
        assert!(json.contains("\"evidence_missing\": 1"));
        assert!(json.contains("\"baseline_debt\": 0"));
    }

    #[test]
    fn json_report_trend_counts_policy_baseline_debt_context() {
        let json = render_json_with_context(
            "audit",
            &[],
            &[],
            false,
            ReportContext {
                inventory_source: "git_tracked",
                baseline_debt_entries: Some(3),
                ..ReportContext::default()
            },
        );

        assert!(json.contains("\"review_items\": 3"));
        assert!(json.contains("\"baseline_debt\": 3"));
    }

    #[test]
    fn json_report_exposes_source_package_context_on_findings() {
        let mut identity = StructuralIdentity::new("rust", "method_call");
        identity.crate_name = Some("parser".to_string());
        let findings = vec![Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("crates/parser/src/lib.rs"),
            span: Some(Span {
                line: 12,
                column: 8,
            }),
            identity,
            message: "unwrap call".to_string(),
        }];

        let json = render_json("audit", &findings, &[], false);

        assert!(json.contains("\"source_package\": \"parser\""));
        assert!(json.contains("\"path\": \"crates/parser/src/lib.rs\""));
    }

    #[test]
    fn sarif_report_emits_non_matched_results_with_locations() {
        let findings = vec![file_finding(
            FindingKind::NonRustFile,
            "shell_script",
            "scripts/new.sh",
        )];
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some(0)),
            MatchOutcome {
                status: MatchStatus::New,
                allow_id: None,
                finding_index: Some(0),
                message: "unreceipted shell script at scripts/new.sh".to_string(),
                score: 0,
            },
        ];

        let sarif =
            render_sarif_with_context("check", &findings, &outcomes, true, context("git_tracked"));

        assert!(sarif.contains("\"version\": \"2.1.0\""));
        assert!(sarif.contains("\"name\": \"cargo-allow\""));
        assert!(sarif.contains("\"ruleId\": \"cargo-allow/new\""));
        assert!(sarif.contains("\"level\": \"error\""));
        assert!(sarif.contains("\"uri\": \"scripts/new.sh\""));
        assert!(sarif.contains("\"startLine\": 1"));
        assert!(sarif.contains("\"source_tree_inventory\""));
        assert!(sarif.contains("\"cargo_commands_not_invoked\""));
        assert!(!sarif.contains("\"ruleId\": \"cargo-allow/matched\""));
    }

    #[test]
    fn sarif_result_properties_include_source_package_context() {
        let mut identity = StructuralIdentity::new("rust", "method_call");
        identity.crate_name = Some("parser".to_string());
        let findings = vec![Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("crates/parser/src/lib.rs"),
            span: Some(Span { line: 4, column: 9 }),
            identity,
            message: "unwrap call".to_string(),
        }];
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: Some(0),
            message: "unreceipted unwrap".to_string(),
            score: 0,
        }];

        let sarif = render_sarif("check", &findings, &outcomes, true);

        assert!(sarif.contains("\"source_package\": \"parser\""));
        assert!(sarif.contains("\"uri\": \"crates/parser/src/lib.rs\""));
    }

    #[test]
    fn receipt_exposes_v1_schema_contract() {
        let json = render_receipt_with_context(
            "check",
            &[],
            true,
            ReportContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(42),
                ..ReportContext::default()
            },
        );
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"schema_id\": \"cargo-allow.receipt.v1\""));
        assert!(json.contains("\"failed\": true"));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 42"));
        assert!(json.contains("\"cargo_metadata_not_invoked\""));
        assert!(json.contains("\"cargo_commands_not_invoked\""));
        assert!(json.contains("\"build_output_not_analyzed\""));
        assert!(json.contains("\"macro_token_tree_contents_not_analyzed\""));
        assert!(json.contains("\"missing_required_field\": 0"));
        assert!(json.contains("\"evidence_missing\": 0"));
    }

    #[test]
    fn schemas_reference_current_contract_ids() {
        let report_schema = include_str!("../../../docs/schemas/report.schema.json");
        let receipt_schema = include_str!("../../../docs/schemas/receipt.schema.json");
        let worklist_schema = include_str!("../../../docs/schemas/worklist.schema.json");
        let list_schema = include_str!("../../../docs/schemas/list.schema.json");
        let explain_schema = include_str!("../../../docs/schemas/explain.schema.json");
        let prune_schema = include_str!("../../../docs/schemas/prune.schema.json");
        let doctor_schema = include_str!("../../../docs/schemas/doctor.schema.json");
        let propose_schema = include_str!("../../../docs/schemas/propose.schema.json");
        assert!(report_schema.contains(REPORT_SCHEMA_ID));
        assert!(receipt_schema.contains(RECEIPT_SCHEMA_ID));
        assert!(worklist_schema.contains(WORKLIST_SCHEMA_ID));
        assert!(list_schema.contains(LIST_SCHEMA_ID));
        assert!(explain_schema.contains(EXPLAIN_SCHEMA_ID));
        assert!(prune_schema.contains(PRUNE_SCHEMA_ID));
        assert!(doctor_schema.contains(DOCTOR_SCHEMA_ID));
        assert!(propose_schema.contains(PROPOSE_SCHEMA_ID));
        assert!(report_schema.contains("\"files_scanned\""));
        assert!(receipt_schema.contains("\"files_scanned\""));
        assert!(list_schema.contains("\"files_scanned\""));
        assert!(explain_schema.contains("\"files_scanned\""));
        assert!(prune_schema.contains("\"files_scanned\""));
        assert!(doctor_schema.contains("\"files_scanned\""));
        assert!(propose_schema.contains("\"files_scanned\""));
        assert!(report_schema.contains("\"root\""));
        assert!(receipt_schema.contains("\"root\""));
        assert!(list_schema.contains("\"root\""));
        assert!(explain_schema.contains("\"root\""));
        assert!(prune_schema.contains("\"root\""));
        assert!(doctor_schema.contains("\"root\""));
        assert!(propose_schema.contains("\"root\""));
        assert!(report_schema.contains("\"source_package\""));
        assert!(list_schema.contains("\"source_package\""));
        assert!(explain_schema.contains("\"source_package\""));
        assert!(report_schema.contains("\"scanner_limitation\""));
        assert!(receipt_schema.contains("\"scanner_limitation\""));
        assert!(list_schema.contains("\"scanner_limitation\""));
        assert!(explain_schema.contains("\"scanner_limitation\""));
        assert!(prune_schema.contains("\"scanner_limitation\""));
        assert!(doctor_schema.contains("\"scanner_limitation\""));
        assert!(propose_schema.contains("\"scanner_limitation\""));
        assert!(report_schema.contains("\"repository_code_not_executed\""));
        assert!(receipt_schema.contains("\"repository_code_not_executed\""));
        assert!(list_schema.contains("\"repository_code_not_executed\""));
        assert!(explain_schema.contains("\"repository_code_not_executed\""));
        assert!(prune_schema.contains("\"repository_code_not_executed\""));
        assert!(doctor_schema.contains("\"repository_code_not_executed\""));
        assert!(propose_schema.contains("\"repository_code_not_executed\""));
        for limitation in SCANNER_LIMITATIONS {
            assert!(report_schema.contains(limitation));
            assert!(receipt_schema.contains(limitation));
            assert!(worklist_schema.contains(limitation));
            assert!(list_schema.contains(limitation));
            assert!(explain_schema.contains(limitation));
            assert!(prune_schema.contains(limitation));
            assert!(doctor_schema.contains(limitation));
            assert!(propose_schema.contains(limitation));
        }
        for claim in CLAIM_BOUNDARY {
            assert!(report_schema.contains(claim));
            assert!(receipt_schema.contains(claim));
            assert!(worklist_schema.contains(claim));
            assert!(list_schema.contains(claim));
            assert!(explain_schema.contains(claim));
            assert!(prune_schema.contains(claim));
            assert!(doctor_schema.contains(claim));
            assert!(propose_schema.contains(claim));
        }
    }

    #[test]
    fn human_report_summarizes_non_rust_inventory() {
        let findings = vec![
            file_finding(FindingKind::NonRustFile, "configuration", ".gitignore"),
            file_finding(
                FindingKind::GeneratedCode,
                "generated_code",
                "schemas/api.yaml",
            ),
        ];
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some(0)),
            outcome(MatchStatus::New, Some(1)),
        ];

        let text = render_human_with_context(
            "audit",
            &findings,
            &outcomes,
            false,
            ReportContext {
                inventory_source: "filesystem_fallback",
                source_tree_root: Some("fixtures/snapshot"),
                inventory_files: Some(2),
                ..ReportContext::default()
            },
        );

        assert!(text.contains(
            "Inventory: source_tree/source_syntax via filesystem_fallback; files scanned: 2"
        ));
        assert!(text.contains("Source tree root: fixtures/snapshot"));
        assert!(text.contains("Non-Rust file inventory:"));
        assert!(text.contains("files scanned              2"));
        assert!(text.contains("new                        1"));
        assert!(text.contains("generated                  1"));
        assert!(text.contains("configuration"));
        assert!(text.contains("generated_code"));
        assert!(text.contains("    matched      configuration            .gitignore"));
        assert!(text.contains("schemas/api.yaml"));
        assert!(text.contains("did not invoke Cargo metadata"));
        assert!(text.contains("repository code"));
    }

    #[test]
    fn markdown_report_summarizes_non_rust_inventory() {
        let findings = vec![file_finding(
            FindingKind::NonRustFile,
            "ci_declarative",
            ".github/workflows/ci.yml",
        )];
        let outcomes = vec![outcome(MatchStatus::Matched, Some(0))];

        let text = render_markdown_with_context(
            "audit",
            &findings,
            &outcomes,
            false,
            ReportContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(1),
                ..ReportContext::default()
            },
        );

        assert!(text.contains(
            "Inventory: `source_tree` / `source_syntax` via `git_tracked`; files scanned: `1`"
        ));
        assert!(text.contains("Source tree root: `H:/Code/Rust/cargo-allow`"));
        assert!(text.contains("## Non-Rust File Inventory"));
        assert!(text.contains("| Files scanned | 1 |"));
        assert!(text.contains("| `ci_declarative` | 1 |"));
        assert!(text.contains("| `matched` | `ci_declarative` | `.github/workflows/ci.yml` |"));
        assert!(!text.contains("## Non-matched outcomes"));
        assert!(text.contains("did not invoke Cargo metadata"));
        assert!(text.contains("proc macros"));
    }

    #[test]
    fn html_report_summarizes_audit_posture() {
        let findings = vec![file_finding(
            FindingKind::NonRustFile,
            "shell_script",
            "scripts/new.sh",
        )];
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: Some(0),
            message: "unreceipted shell script at scripts/new.sh".to_string(),
            score: 0,
        }];

        let html =
            render_html_with_context("audit", &findings, &outcomes, true, context("git_tracked"));

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("<h1>cargo-allow audit</h1>"));
        assert!(html.contains("Result: failed"));
        assert!(html.contains("<h2>Audit Summary</h2>"));
        assert!(html.contains("<h2>Non-Rust File Inventory</h2>"));
        assert!(html.contains("<code>new</code>"));
        assert!(html.contains("<code>scripts/new.sh</code>"));
        assert!(html.contains("did not invoke Cargo metadata"));
    }

    #[test]
    fn markdown_audit_report_includes_review_summary() {
        let findings = vec![
            file_finding(FindingKind::NonRustFile, "shell_script", "scripts/new.sh"),
            file_finding(FindingKind::Unsafe, "unsafe_block", "src/ffi.rs"),
        ];
        let outcomes = vec![
            MatchOutcome {
                status: MatchStatus::New,
                allow_id: None,
                finding_index: Some(0),
                message: "unreceipted shell script at scripts/new.sh".to_string(),
                score: 0,
            },
            MatchOutcome {
                status: MatchStatus::EvidenceMissing,
                allow_id: Some("allow-unsafe-ffi".to_string()),
                finding_index: Some(1),
                message: "allow-unsafe-ffi matched unsafe finding but has no evidence".to_string(),
                score: 0,
            },
        ];

        let text = render_markdown_with_context(
            "audit",
            &findings,
            &outcomes,
            false,
            context("git_tracked"),
        );

        assert!(text.contains("## Audit Summary"));
        assert!(text.contains("| Match outcomes | 2 |"));
        assert!(text.contains("| Review items | 2 |"));
        assert!(text.contains("| New unreceipted | 1 |"));
        assert!(text.contains("| Evidence gaps | 1 |"));
        assert!(
            text.contains(
                "Recommended next step: review the queue below before tightening policy."
            )
        );
        assert!(text.contains("## Audit Review Queue"));
        assert!(text.contains("- `new`: unreceipted shell script at scripts/new.sh"));
        assert!(text.contains(
            "- `evidence_missing`: allow-unsafe-ffi matched unsafe finding but has no evidence"
        ));
    }

    #[test]
    fn markdown_audit_report_counts_policy_baseline_debt_context() {
        let text = render_markdown_with_context(
            "audit",
            &[],
            &[],
            false,
            ReportContext {
                inventory_source: "git_tracked",
                baseline_debt_entries: Some(3),
                ..ReportContext::default()
            },
        );

        assert!(text.contains("| Review items | 3 |"));
        assert!(text.contains("| Baseline debt | 3 |"));
        assert!(text.contains("cargo-allow worklist --format json"));
        assert!(!text.contains("## Audit Review Queue"));
    }

    #[test]
    fn text_reports_include_review_due_and_invalid_selector_counts() {
        let outcomes = vec![
            MatchOutcome {
                status: MatchStatus::ReviewDue,
                allow_id: Some("allow-review".to_string()),
                finding_index: None,
                message: "allow-review is due for review".to_string(),
                score: 0,
            },
            MatchOutcome {
                status: MatchStatus::InvalidSelector,
                allow_id: Some("allow-invalid".to_string()),
                finding_index: None,
                message: "allow-invalid selector is invalid".to_string(),
                score: 0,
            },
        ];

        let human = render_human("check", &[], &outcomes, true);
        let markdown = render_markdown("check", &[], &outcomes, true);

        assert!(human.contains("review_due"));
        assert!(human.contains("invalid_selector"));
        assert!(markdown.contains("| `review_due` | 1 |"));
        assert!(markdown.contains("| `invalid_selector` | 1 |"));
    }

    fn file_finding(kind: FindingKind, family: &str, path: &str) -> Finding {
        Finding {
            kind,
            family: Some(family.to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: "tracked non-Rust file".to_string(),
        }
    }

    fn outcome(status: MatchStatus, finding_index: Option<usize>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: None,
            finding_index,
            message: String::new(),
            score: 0,
        }
    }
}
