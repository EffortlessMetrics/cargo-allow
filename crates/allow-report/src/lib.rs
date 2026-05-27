use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, json_escape, normalize_path};
use std::collections::BTreeMap;

pub const REPORT_SCHEMA_VERSION: u32 = 1;
pub const REPORT_SCHEMA_ID: &str = "cargo-allow.report.v1";
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const RECEIPT_SCHEMA_ID: &str = "cargo-allow.receipt.v1";

const CLAIM_BOUNDARY: &[&str] = &[
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

const SCANNER_LIMITATIONS: &[&str] = &[
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
}

impl Default for ReportContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
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
        "Inventory: source_tree/source_syntax via {}\n",
        context.inventory_source
    ));
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
        "Inventory: `source_tree` / `source_syntax` via `{}`\n\n",
        json_escape(context.inventory_source)
    ));
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
        render_audit_summary_markdown(&summary, outcomes, &mut out);
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

fn render_audit_summary_markdown(summary: &Summary, outcomes: &[MatchOutcome], out: &mut String) {
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
    let review_items = review_statuses
        .iter()
        .map(|status| summary.count(*status))
        .sum::<usize>();
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
    out.push_str(&format!(
        "| Baseline debt | {} |\n",
        summary.count(MatchStatus::BaselineDebt)
    ));
    if review_items == 0 {
        out.push_str("\nRecommended next step: keep `cargo-allow check --mode no-new` in CI.\n");
    } else {
        out.push_str("\nRecommended next step: review the queue below before tightening policy.\n");
    }

    let queue = outcomes
        .iter()
        .filter(|outcome| review_statuses.contains(&outcome.status))
        .take(20)
        .collect::<Vec<_>>();
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
    out.push_str("  \"inventory\": {\n");
    out.push_str("    \"scope\": \"source_tree\",\n");
    out.push_str("    \"scanner\": \"source_syntax\",\n");
    out.push_str(&format!(
        "    \"source\": \"{}\"\n",
        json_escape(context.inventory_source)
    ));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"findings\": {},\n", findings.len()));
    out.push_str(&format!("    \"outcomes\": {},\n", summary.total));
    out.push_str(&render_counts_fields(&summary, "    "));
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
        "{{\n  \"schema_version\": {RECEIPT_SCHEMA_VERSION},\n  \"schema_id\": \"{RECEIPT_SCHEMA_ID}\",\n  \"tool\": \"cargo-allow\",\n  \"command\": \"{}\",\n  \"status\": \"{}\",\n  \"failed\": {},\n  \"claim_boundary\": {},\n  \"scanner_limitations\": {},\n  \"inventory\": {{\n    \"scope\": \"source_tree\",\n    \"scanner\": \"source_syntax\",\n    \"source\": \"{}\"\n  }},\n  \"counts\": {{\n{}  }}\n}}\n",
        json_escape(command),
        if failed { "failed" } else { "passed" },
        bool_json(failed),
        json_string_array(CLAIM_BOUNDARY),
        json_string_array(SCANNER_LIMITATIONS),
        json_escape(context.inventory_source),
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

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('`', "\\`")
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

    #[test]
    fn json_contains_claim_boundary() {
        let json = render_json("audit", &[], &[], false);
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
        let json = render_json("audit", &[], &[], false);
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"schema_id\": \"cargo-allow.report.v1\""));
        assert!(json.contains("\"failed\": false"));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"scope\": \"source_tree\""));
        assert!(json.contains("\"scanner\": \"source_syntax\""));
        assert!(json.contains("\"source\": \"unknown\""));
        assert!(json.contains("\"review_due\": 0"));
        assert!(json.contains("\"baseline_debt\": 0"));
    }

    #[test]
    fn receipt_exposes_v1_schema_contract() {
        let json = render_receipt_with_context(
            "check",
            &[],
            true,
            ReportContext {
                inventory_source: "git_tracked",
            },
        );
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"schema_id\": \"cargo-allow.receipt.v1\""));
        assert!(json.contains("\"failed\": true"));
        assert!(json.contains("\"source\": \"git_tracked\""));
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
        assert!(report_schema.contains(REPORT_SCHEMA_ID));
        assert!(receipt_schema.contains(RECEIPT_SCHEMA_ID));
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
            },
        );

        assert!(text.contains("Inventory: source_tree/source_syntax via filesystem_fallback"));
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
            },
        );

        assert!(text.contains("Inventory: `source_tree` / `source_syntax` via `git_tracked`"));
        assert!(text.contains("## Non-Rust File Inventory"));
        assert!(text.contains("| Files scanned | 1 |"));
        assert!(text.contains("| `ci_declarative` | 1 |"));
        assert!(text.contains("| `matched` | `ci_declarative` | `.github/workflows/ci.yml` |"));
        assert!(!text.contains("## Non-matched outcomes"));
        assert!(text.contains("did not invoke Cargo metadata"));
        assert!(text.contains("proc macros"));
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
            ReportContext {
                inventory_source: "git_tracked",
            },
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
