use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchOutcome, MatchStatus, Selector,
    StructuralIdentity, json_escape, normalize_path,
};
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
pub const ADD_SCHEMA_VERSION: u32 = 1;
pub const ADD_SCHEMA_ID: &str = "cargo-allow.add.v1";
pub const MIGRATE_SCHEMA_VERSION: u32 = 1;
pub const MIGRATE_SCHEMA_ID: &str = "cargo-allow.migrate.v1";

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
pub struct InventoryContext<'a> {
    pub scope: &'a str,
    pub scanner: &'a str,
    pub source: &'a str,
    pub root: Option<&'a str>,
    pub files_scanned: Option<usize>,
}

impl<'a> InventoryContext<'a> {
    pub const fn new(
        scope: &'a str,
        scanner: &'a str,
        source: &'a str,
        root: Option<&'a str>,
        files_scanned: Option<usize>,
    ) -> Self {
        Self {
            scope,
            scanner,
            source,
            root,
            files_scanned,
        }
    }

    pub const fn source_syntax(
        source: &'a str,
        root: Option<&'a str>,
        files_scanned: Option<usize>,
    ) -> Self {
        Self::new("source_tree", "source_syntax", source, root, files_scanned)
    }
}

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

impl<'a> From<ReportContext<'a>> for InventoryContext<'a> {
    fn from(context: ReportContext<'a>) -> Self {
        Self::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PruneModeContext<'a> {
    pub explicit_dry_run: bool,
    pub write_requested: bool,
    pub written_path: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct PruneCandidate<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub owner: &'a str,
    pub classification: &'a str,
    pub scope: &'a str,
    pub reason: &'a str,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListFilters<'a> {
    pub kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub status: Option<&'a str>,
    pub expired: bool,
    pub review_due: bool,
    pub stale: bool,
    pub baseline_debt: bool,
    pub broad_scope: bool,
    pub missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ListRow<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub matches: usize,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub owner: &'a str,
    pub classification: &'a str,
    pub scope: &'a str,
    pub source_package: Option<&'a str>,
    pub evidence_count: usize,
    pub review_after: Option<&'a str>,
    pub expires: Option<&'a str>,
    pub reason: &'a str,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EvidenceReference<'a> {
    pub raw: &'a str,
    pub prefix: Option<&'a str>,
    pub target: Option<&'a str>,
    pub status: &'a str,
    pub message: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ExplainReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub current_findings: &'a [Finding],
    pub match_outcomes: &'a [MatchOutcome],
    pub evidence_references: &'a [EvidenceReference<'a>],
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorklistFilters<'a> {
    pub kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub item_kind: Option<&'a str>,
    pub status: Option<&'a str>,
    pub allow_id: Option<&'a str>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub baseline_debt: bool,
    pub broad_scope: bool,
    pub risk: Option<&'a str>,
    pub difficulty: Option<&'a str>,
    pub missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct WorklistItem<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub exception_kind: Option<&'a str>,
    pub family: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub classification: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub created: Option<&'a str>,
    pub review_after: Option<&'a str>,
    pub expires: Option<&'a str>,
    pub evidence_count: Option<usize>,
    pub risk: &'a str,
    pub difficulty: &'a str,
    pub status: &'a str,
    pub allow_id: Option<&'a str>,
    pub finding_index: Option<usize>,
    pub path: Option<&'a str>,
    pub source_package: Option<&'a str>,
    pub message: &'a str,
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct DoctorReport<'a> {
    pub source_tree_root: &'a str,
    pub root_discovery: &'a str,
    pub config_path: Option<&'a str>,
    pub inventory_source: &'a str,
    pub files_scanned: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ProposeReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub kind: Option<&'a str>,
    pub expires: &'a str,
    pub policy_output: Option<&'a str>,
    pub force: bool,
    pub findings_scanned: usize,
    pub baseline_debt_entries_proposed: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct AddReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub selected_finding: &'a Finding,
    pub policy_output: Option<&'a str>,
    pub force: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct MigrateReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub input_kind: &'a str,
    pub input_path: &'a str,
    pub output_path: &'a str,
    pub force: bool,
    pub allow_entries: usize,
    pub baseline_debt: usize,
    pub unsafe_entries: usize,
    pub entries_with_evidence: usize,
    pub notes: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffPostureSummary {
    pub current_failures: usize,
    pub new_findings: usize,
    pub removed_findings: usize,
    pub policy_failures: usize,
    pub policy_review_items: usize,
    pub policy_improvements: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffFindingChange<'a> {
    pub change: &'a str,
    pub key: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub path: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffPolicyChange<'a> {
    pub severity: &'a str,
    pub allow_id: &'a str,
    pub kind: &'a str,
    pub message: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffReport<'a> {
    pub net_posture: &'a str,
    pub reviewer_action: &'a str,
    pub summary: DiffPostureSummary,
    pub finding_changes: &'a [DiffFindingChange<'a>],
    pub policy_changes: &'a [DiffPolicyChange<'a>],
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
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(context.into(), "  "));
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
    out.push_str(&render_inventory_json(context.into(), "        "));
    out.push_str(",\n");
    out.push_str(&format!(
        "        \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "        \"scanner_limitations\": {}\n",
        render_scanner_limitations_json()
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
        render_claim_boundary_json(),
        render_scanner_limitations_json(),
        render_inventory_json(context.into(), "  "),
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

fn option_u32_json(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn option_usize_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_string_array<T: AsRef<str>>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value.as_ref())))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn render_claim_boundary_json() -> String {
    json_string_array(CLAIM_BOUNDARY)
}

pub fn render_scanner_limitations_json() -> String {
    json_string_array(SCANNER_LIMITATIONS)
}

pub fn render_inventory_json(context: InventoryContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"scope\": \"{}\",\n",
        json_escape(context.scope)
    ));
    out.push_str(&format!(
        "{indent}  \"scanner\": \"{}\",\n",
        json_escape(context.scanner)
    ));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.source)
    ));
    if let Some(root) = context.root {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.files_scanned {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"files_scanned\": {files}"));
    }
    out.push('\n');
    out.push_str(&format!("{indent}}}"));
    out
}

pub fn render_allow_entry_json(entry: &AllowEntry, indent: &str) -> String {
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!("{indent}  \"kind\": \"{}\",\n", entry.kind));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json(entry.family.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"scope\": \"{}\",\n",
        json_escape(&entry.path_or_glob())
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json(path.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"glob\": {},\n",
        option_json(entry.glob.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": \"{}\",\n",
        json_escape(&entry.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": \"{}\",\n",
        json_escape(&entry.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"reason\": \"{}\",\n",
        json_escape(&entry.reason)
    ));
    out.push_str(&format!(
        "{indent}  \"evidence\": {},\n",
        json_string_array(&entry.evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"links\": {},\n",
        json_string_array(&entry.links)
    ));
    out.push_str(&format!(
        "{indent}  \"occurrence_limit\": {},\n",
        option_u32_json(entry.occurrence_limit)
    ));
    out.push_str(&format!(
        "{indent}  \"lifecycle\": {},\n",
        lifecycle_json(&entry.lifecycle, indent)
    ));
    out.push_str(&format!(
        "{indent}  \"selector\": {},\n",
        render_selector_json(&entry.selector, indent)
    ));
    out.push_str(&format!(
        "{indent}  \"last_seen\": {}\n",
        render_last_seen_json(entry.last_seen.as_ref(), indent)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn lifecycle_json(lifecycle: &Lifecycle, indent: &str) -> String {
    format!(
        "{{\n{indent}    \"created\": {},\n{indent}    \"review_after\": {},\n{indent}    \"expires\": {}\n{indent}  }}",
        option_json(lifecycle.created.as_deref()),
        option_json(lifecycle.review_after.as_deref()),
        option_json(lifecycle.expires.as_deref())
    )
}

pub fn render_selector_json(selector: &Selector, indent: &str) -> String {
    format!(
        "{{\n{indent}    \"ast_kind\": {},\n{indent}    \"container\": {},\n{indent}    \"callee\": {},\n{indent}    \"macro_name\": {},\n{indent}    \"lint\": {},\n{indent}    \"symbol\": {},\n{indent}    \"receiver_fingerprint\": {},\n{indent}    \"target_fingerprint\": {},\n{indent}    \"normalized_snippet_hash\": {},\n{indent}    \"line_hint\": {},\n{indent}    \"glob\": {}\n{indent}  }}",
        option_json(selector.ast_kind.as_deref()),
        option_json(selector.container.as_deref()),
        option_json(selector.callee.as_deref()),
        option_json(selector.macro_name.as_deref()),
        option_json(selector.lint.as_deref()),
        option_json(selector.symbol.as_deref()),
        option_json(selector.receiver_fingerprint.as_deref()),
        option_json(selector.target_fingerprint.as_deref()),
        option_json(selector.normalized_snippet_hash.as_deref()),
        option_u32_json(selector.line_hint),
        option_json(selector.glob.as_deref())
    )
}

pub fn render_last_seen_json(last_seen: Option<&LastSeen>, indent: &str) -> String {
    last_seen
        .map(|last_seen| {
            format!(
                "{{\n{indent}    \"line\": {},\n{indent}    \"column\": {}\n{indent}  }}",
                last_seen.line, last_seen.column
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

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
        option_json(source_package_name(finding).as_deref()),
        structural_identity_json(&finding.identity, indent),
        json_escape(&finding.message)
    )
}

fn finding_location_text(finding: &Finding) -> String {
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

fn source_package_name(finding: &Finding) -> Option<String> {
    finding
        .identity
        .crate_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

pub fn render_prune_json(
    candidates: &[PruneCandidate<'_>],
    mode: PruneModeContext<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema_version\": {PRUNE_SCHEMA_VERSION},\n"));
    out.push_str(&format!("  \"schema_id\": \"{PRUNE_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"prune\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(inventory, "  "));
    out.push_str(",\n");
    out.push_str("  \"mode\": {\n");
    out.push_str(&format!(
        "    \"dry_run\": {},\n",
        bool_json(!mode.write_requested)
    ));
    out.push_str(&format!(
        "    \"write_requested\": {},\n",
        bool_json(mode.write_requested)
    ));
    out.push_str(&format!(
        "    \"explicit_dry_run\": {},\n",
        bool_json(mode.explicit_dry_run)
    ));
    out.push_str(&format!(
        "    \"written_path\": {}\n",
        option_json(mode.written_path)
    ));
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"stale_entries\": {}\n  }},\n",
        candidates.len()
    ));
    out.push_str("  \"stale_entries\": [\n");
    for (index, candidate) in candidates.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_prune_candidate_json(candidate, "  "));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

pub fn render_list_json(
    rows: &[ListRow<'_>],
    filters: ListFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema_version\": {LIST_SCHEMA_VERSION},\n"));
    out.push_str(&format!("  \"schema_id\": \"{LIST_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"list\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(inventory, "  "));
    out.push_str(",\n");
    out.push_str("  \"filters\": ");
    out.push_str(&render_list_filters_json(filters, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"allow_entries\": {}\n  }},\n",
        rows.len()
    ));
    out.push_str("  \"allow_entries\": [\n");
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_list_row_json(row));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

pub fn render_worklist_json(
    items: &[WorklistItem<'_>],
    filters: WorklistFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {WORKLIST_SCHEMA_VERSION},\n"
    ));
    out.push_str(&format!("  \"schema_id\": \"{WORKLIST_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"worklist\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(inventory, "  "));
    out.push_str(",\n");
    out.push_str("  \"filters\": ");
    out.push_str(&render_worklist_filters_json(filters, "  "));
    out.push_str(",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"work_items\": {},\n", items.len()));
    out.push_str(&format!(
        "    \"high\": {},\n",
        worklist_risk_count(items, "high")
    ));
    out.push_str(&format!(
        "    \"medium\": {},\n",
        worklist_risk_count(items, "medium")
    ));
    out.push_str(&format!(
        "    \"low\": {},\n",
        worklist_risk_count(items, "low")
    ));
    out.push_str(&format!(
        "    \"small_difficulty\": {},\n",
        worklist_difficulty_count(items, "small")
    ));
    out.push_str(&format!(
        "    \"medium_difficulty\": {}\n",
        worklist_difficulty_count(items, "medium")
    ));
    out.push_str("  },\n");
    out.push_str("  \"work_items\": [\n");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_work_item_json(item));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

pub fn render_doctor_json(facts: DoctorReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema_version\": {DOCTOR_SCHEMA_VERSION},\n"));
    out.push_str(&format!("  \"schema_id\": \"{DOCTOR_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"doctor\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"root\": {\n");
    out.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(facts.source_tree_root)
    ));
    out.push_str(&format!(
        "    \"discovery\": \"{}\"\n",
        json_escape(facts.root_discovery)
    ));
    out.push_str("  },\n");
    out.push_str("  \"config\": {\n");
    out.push_str(&format!(
        "    \"found\": {},\n",
        bool_json(facts.config_path.is_some())
    ));
    out.push_str(&format!(
        "    \"path\": {}\n",
        option_json(facts.config_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"inventory\": {\n");
    out.push_str("    \"scope\": \"source_tree\",\n");
    out.push_str("    \"scanner\": \"source_syntax\",\n");
    out.push_str(&format!(
        "    \"source\": \"{}\",\n",
        json_escape(facts.inventory_source)
    ));
    out.push_str(&format!("    \"files_scanned\": {}\n", facts.files_scanned));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

pub fn render_propose_json(report: ProposeReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {PROPOSE_SCHEMA_VERSION},\n"
    ));
    out.push_str(&format!("  \"schema_id\": \"{PROPOSE_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"propose\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(report.inventory, "  "));
    out.push_str(",\n");
    out.push_str("  \"options\": {\n");
    out.push_str(&format!("    \"kind\": {},\n", option_json(report.kind)));
    out.push_str(&format!(
        "    \"expires\": \"{}\",\n",
        json_escape(report.expires)
    ));
    out.push_str(&format!(
        "    \"policy_output\": {},\n",
        option_json(report.policy_output)
    ));
    out.push_str(&format!("    \"force\": {}\n", bool_json(report.force)));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"findings_scanned\": {},\n",
        report.findings_scanned
    ));
    out.push_str(&format!(
        "    \"baseline_debt_entries_proposed\": {}\n",
        report.baseline_debt_entries_proposed
    ));
    out.push_str("  },\n");
    out.push_str("  \"generated_entry_defaults\": {\n");
    out.push_str("    \"owner\": \"unowned\",\n");
    out.push_str("    \"classification\": \"baseline_debt\",\n");
    out.push_str("    \"reason\": \"Generated by cargo-allow propose; requires human review.\",\n");
    out.push_str(&format!(
        "    \"expires\": \"{}\"\n",
        json_escape(report.expires)
    ));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

pub fn render_explain_json(report: ExplainReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {EXPLAIN_SCHEMA_VERSION},\n"
    ));
    out.push_str(&format!("  \"schema_id\": \"{EXPLAIN_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"explain\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(report.inventory, "  "));
    out.push_str(",\n");
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

pub fn render_diff_json_with_posture(report_json: &str, report: DiffReport<'_>) -> Option<String> {
    let diff_json = render_diff_posture_json(report);
    let trimmed = report_json.trim_end();
    trimmed
        .strip_suffix('}')
        .map(|prefix| format!("{prefix},\n  \"diff\": {diff_json}\n}}\n"))
}

fn render_diff_posture_json(report: DiffReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "    \"net_posture\": \"{}\",\n",
        json_escape(report.net_posture)
    ));
    out.push_str(&format!(
        "    \"reviewer_action\": \"{}\",\n",
        json_escape(report.reviewer_action)
    ));
    out.push_str("    \"summary\": {\n");
    out.push_str(&format!(
        "      \"current_failures\": {},\n",
        report.summary.current_failures
    ));
    out.push_str(&format!(
        "      \"new_findings\": {},\n",
        report.summary.new_findings
    ));
    out.push_str(&format!(
        "      \"removed_findings\": {},\n",
        report.summary.removed_findings
    ));
    out.push_str(&format!(
        "      \"policy_failures\": {},\n",
        report.summary.policy_failures
    ));
    out.push_str(&format!(
        "      \"policy_review_items\": {},\n",
        report.summary.policy_review_items
    ));
    out.push_str(&format!(
        "      \"policy_improvements\": {}\n",
        report.summary.policy_improvements
    ));
    out.push_str("    },\n");
    out.push_str("    \"finding_changes\": [\n");
    for (index, change) in report.finding_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"change\": \"{}\", ", json_escape(change.change)));
        out.push_str(&format!("\"key\": \"{}\", ", json_escape(change.key)));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"family\": {}, ", option_json(change.family)));
        out.push_str(&format!("\"path\": \"{}\"", json_escape(change.path)));
        out.push('}');
    }
    out.push_str("\n    ],\n");
    out.push_str("    \"policy_changes\": [\n");
    for (index, change) in report.policy_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!(
            "\"severity\": \"{}\", ",
            json_escape(change.severity)
        ));
        out.push_str(&format!(
            "\"allow_id\": \"{}\", ",
            json_escape(change.allow_id)
        ));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"message\": \"{}\"", json_escape(change.message)));
        out.push('}');
    }
    out.push_str("\n    ]\n");
    out.push_str("  }");
    out
}

pub fn render_add_json(report: AddReport<'_>) -> String {
    let entry = report.entry;
    let selected_finding = report.selected_finding;
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema_version\": {ADD_SCHEMA_VERSION},\n"));
    out.push_str(&format!("  \"schema_id\": \"{ADD_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"add\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(report.inventory, "  "));
    out.push_str(",\n");
    out.push_str("  \"options\": {\n");
    out.push_str(&format!(
        "    \"policy_output\": {},\n",
        option_json(report.policy_output)
    ));
    out.push_str(&format!("    \"force\": {}\n", bool_json(report.force)));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"entry_id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!(
        "    \"selected_finding\": \"{}\",\n",
        json_escape(&finding_location_text(selected_finding))
    ));
    out.push_str("    \"human_review_required\": true\n");
    out.push_str("  },\n");
    out.push_str("  \"allow_entry\": {\n");
    out.push_str(&format!("    \"id\": \"{}\",\n", json_escape(&entry.id)));
    out.push_str(&format!("    \"kind\": \"{}\",\n", entry.kind));
    out.push_str(&format!(
        "    \"family\": {},\n",
        option_json(entry.family.as_deref())
    ));
    out.push_str(&format!(
        "    \"path\": {},\n",
        option_json(path.as_deref())
    ));
    out.push_str(&format!(
        "    \"glob\": {},\n",
        option_json(entry.glob.as_deref())
    ));
    out.push_str(&format!(
        "    \"owner\": \"{}\",\n",
        json_escape(&entry.owner)
    ));
    out.push_str(&format!(
        "    \"classification\": \"{}\",\n",
        json_escape(&entry.classification)
    ));
    out.push_str(&format!(
        "    \"reason\": \"{}\",\n",
        json_escape(&entry.reason)
    ));
    out.push_str(&format!(
        "    \"review_after\": {},\n",
        option_json(entry.lifecycle.review_after.as_deref())
    ));
    out.push_str(&format!(
        "    \"expires\": {},\n",
        option_json(entry.lifecycle.expires.as_deref())
    ));
    out.push_str(&format!(
        "    \"evidence_count\": {},\n",
        entry.evidence.len()
    ));
    out.push_str("    \"selector\": ");
    out.push_str(&render_selector_json(&entry.selector, "    "));
    out.push_str(",\n");
    out.push_str("    \"last_seen\": ");
    out.push_str(&render_last_seen_json(entry.last_seen.as_ref(), "    "));
    out.push_str("\n  },\n");
    out.push_str("  \"selected_finding\": ");
    out.push_str(&render_explain_finding_json(
        selected_finding,
        "selected",
        "  ",
    ));
    out.push_str("\n}\n");
    out
}

pub fn render_migrate_json(report: MigrateReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {MIGRATE_SCHEMA_VERSION},\n"
    ));
    out.push_str(&format!("  \"schema_id\": \"{MIGRATE_SCHEMA_ID}\",\n"));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"migrate\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(report.inventory, "  "));
    out.push_str(",\n");
    out.push_str("  \"input\": {\n");
    out.push_str(&format!(
        "    \"kind\": \"{}\",\n",
        json_escape(report.input_kind)
    ));
    out.push_str(&format!(
        "    \"path\": \"{}\"\n",
        json_escape(report.input_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"output\": {\n");
    out.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(report.output_path)
    ));
    out.push_str(&format!("    \"force\": {}\n", bool_json(report.force)));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"allow_entries\": {},\n",
        report.allow_entries
    ));
    out.push_str(&format!(
        "    \"baseline_debt\": {},\n",
        report.baseline_debt
    ));
    out.push_str(&format!(
        "    \"unsafe_entries\": {},\n",
        report.unsafe_entries
    ));
    out.push_str(&format!(
        "    \"entries_with_evidence\": {}\n",
        report.entries_with_evidence
    ));
    out.push_str("  },\n");
    out.push_str(&format!("  \"notes\": \"{}\"\n", json_escape(report.notes)));
    out.push_str("}\n");
    out
}

fn render_work_item_json(item: &WorklistItem<'_>) -> String {
    let mut out = String::new();
    out.push_str("    {\n");
    out.push_str(&format!("      \"id\": \"{}\",\n", json_escape(item.id)));
    out.push_str(&format!(
        "      \"kind\": \"{}\",\n",
        json_escape(item.kind)
    ));
    out.push_str(&format!(
        "      \"exception_kind\": {},\n",
        option_json(item.exception_kind)
    ));
    out.push_str(&format!(
        "      \"family\": {},\n",
        option_json(item.family)
    ));
    out.push_str(&format!("      \"owner\": {},\n", option_json(item.owner)));
    out.push_str(&format!(
        "      \"classification\": {},\n",
        option_json(item.classification)
    ));
    out.push_str(&format!(
        "      \"reason\": {},\n",
        option_json(item.reason)
    ));
    out.push_str(&format!(
        "      \"created\": {},\n",
        option_json(item.created)
    ));
    out.push_str(&format!(
        "      \"review_after\": {},\n",
        option_json(item.review_after)
    ));
    out.push_str(&format!(
        "      \"expires\": {},\n",
        option_json(item.expires)
    ));
    out.push_str(&format!(
        "      \"evidence_count\": {},\n",
        item.evidence_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "      \"risk\": \"{}\",\n",
        json_escape(item.risk)
    ));
    out.push_str(&format!(
        "      \"difficulty\": \"{}\",\n",
        json_escape(item.difficulty)
    ));
    out.push_str(&format!(
        "      \"status\": \"{}\",\n",
        json_escape(item.status)
    ));
    out.push_str(&format!(
        "      \"allow_id\": {},\n",
        option_json(item.allow_id)
    ));
    out.push_str(&format!(
        "      \"finding_index\": {},\n",
        item.finding_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!("      \"path\": {},\n", option_json(item.path)));
    out.push_str(&format!(
        "      \"source_package\": {},\n",
        option_json(item.source_package)
    ));
    out.push_str(&format!(
        "      \"message\": \"{}\",\n",
        json_escape(item.message)
    ));
    out.push_str(&format!(
        "      \"suggested_actions\": {},\n",
        json_string_array(item.suggested_actions)
    ));
    out.push_str(&format!(
        "      \"proof_commands\": {}\n",
        json_string_array(item.proof_commands)
    ));
    out.push_str("    }");
    out
}

fn render_worklist_filters_json(filters: WorklistFilters<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"kind\": {},\n",
        option_json(filters.kind)
    ));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json(filters.family)
    ));
    out.push_str(&format!(
        "{indent}  \"item_kind\": {},\n",
        option_json(filters.item_kind)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json(filters.status)
    ));
    out.push_str(&format!(
        "{indent}  \"allow_id\": {},\n",
        option_json(filters.allow_id)
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json(filters.path)
    ));
    out.push_str(&format!(
        "{indent}  \"source_package\": {},\n",
        option_json(filters.source_package)
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": {},\n",
        option_json(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json(filters.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"baseline_debt\": {},\n",
        bool_json(filters.baseline_debt)
    ));
    out.push_str(&format!(
        "{indent}  \"broad_scope\": {},\n",
        bool_json(filters.broad_scope)
    ));
    out.push_str(&format!(
        "{indent}  \"risk\": {},\n",
        option_json(filters.risk)
    ));
    out.push_str(&format!(
        "{indent}  \"difficulty\": {},\n",
        option_json(filters.difficulty)
    ));
    out.push_str(&format!(
        "{indent}  \"missing_evidence\": {}\n",
        bool_json(filters.missing_evidence)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn worklist_risk_count(items: &[WorklistItem<'_>], risk: &str) -> usize {
    items.iter().filter(|item| item.risk == risk).count()
}

fn worklist_difficulty_count(items: &[WorklistItem<'_>], difficulty: &str) -> usize {
    items
        .iter()
        .filter(|item| item.difficulty == difficulty)
        .count()
}

fn render_list_row_json(row: &ListRow<'_>) -> String {
    let mut out = String::new();
    out.push_str("    {\n");
    out.push_str(&format!("      \"id\": \"{}\",\n", json_escape(row.id)));
    out.push_str(&format!(
        "      \"status\": \"{}\",\n",
        json_escape(row.status)
    ));
    out.push_str(&format!("      \"matches\": {},\n", row.matches));
    out.push_str(&format!("      \"kind\": \"{}\",\n", json_escape(row.kind)));
    out.push_str(&format!("      \"family\": {},\n", option_json(row.family)));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(row.owner)
    ));
    out.push_str(&format!(
        "      \"classification\": \"{}\",\n",
        json_escape(row.classification)
    ));
    out.push_str(&format!(
        "      \"scope\": \"{}\",\n",
        json_escape(row.scope)
    ));
    out.push_str(&format!(
        "      \"source_package\": {},\n",
        option_json(row.source_package)
    ));
    out.push_str(&format!(
        "      \"evidence_count\": {},\n",
        row.evidence_count
    ));
    out.push_str(&format!(
        "      \"review_after\": {},\n",
        option_json(row.review_after)
    ));
    out.push_str(&format!(
        "      \"expires\": {},\n",
        option_json(row.expires)
    ));
    out.push_str(&format!(
        "      \"reason\": \"{}\"\n",
        json_escape(row.reason)
    ));
    out.push_str("    }");
    out
}

fn render_list_filters_json(filters: ListFilters<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"kind\": {},\n",
        option_json(filters.kind)
    ));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json(filters.family)
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": {},\n",
        option_json(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json(filters.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json(filters.path)
    ));
    out.push_str(&format!(
        "{indent}  \"source_package\": {},\n",
        option_json(filters.source_package)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json(filters.status)
    ));
    out.push_str(&format!(
        "{indent}  \"expired\": {},\n",
        bool_json(filters.expired)
    ));
    out.push_str(&format!(
        "{indent}  \"review_due\": {},\n",
        bool_json(filters.review_due)
    ));
    out.push_str(&format!(
        "{indent}  \"stale\": {},\n",
        bool_json(filters.stale)
    ));
    out.push_str(&format!(
        "{indent}  \"baseline_debt\": {},\n",
        bool_json(filters.baseline_debt)
    ));
    out.push_str(&format!(
        "{indent}  \"broad_scope\": {},\n",
        bool_json(filters.broad_scope)
    ));
    out.push_str(&format!(
        "{indent}  \"missing_evidence\": {}\n",
        bool_json(filters.missing_evidence)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn render_prune_candidate_json(candidate: &PruneCandidate<'_>, indent: &str) -> String {
    format!(
        "{indent}  {{\n{indent}    \"id\": \"{}\",\n{indent}    \"kind\": \"{}\",\n{indent}    \"family\": {},\n{indent}    \"owner\": \"{}\",\n{indent}    \"classification\": \"{}\",\n{indent}    \"scope\": \"{}\",\n{indent}    \"reason\": \"{}\"\n{indent}  }}",
        json_escape(candidate.id),
        json_escape(candidate.kind),
        option_json(candidate.family),
        json_escape(candidate.owner),
        json_escape(candidate.classification),
        json_escape(candidate.scope),
        json_escape(candidate.reason)
    )
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
    use allow_core::{
        AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
    };
    use std::path::PathBuf;

    fn context(source: &'static str) -> ReportContext<'static> {
        ReportContext {
            inventory_source: source,
            ..ReportContext::default()
        }
    }

    #[test]
    fn artifact_contract_helpers_render_source_tree_inventory() {
        let inventory = render_inventory_json(
            InventoryContext::new(
                "source_tree",
                "policy_migration",
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(76),
            ),
            "  ",
        );

        assert!(render_claim_boundary_json().contains("source_tree_inventory"));
        assert!(render_scanner_limitations_json().contains("repository_code_not_executed"));
        assert!(inventory.contains("\"scope\": \"source_tree\""));
        assert!(inventory.contains("\"scanner\": \"policy_migration\""));
        assert!(inventory.contains("\"source\": \"git_tracked\""));
        assert!(inventory.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(inventory.contains("\"files_scanned\": 76"));
    }

    #[test]
    fn policy_and_finding_json_helpers_render_current_contract() {
        let entry = AllowEntry {
            id: "allow-json".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("crates\\parser\\src\\lib.rs")),
            glob: None,
            owner: "parser".to_string(),
            classification: "baseline_debt".to_string(),
            reason: "generated baseline".to_string(),
            evidence: vec!["test:parser_handles_empty".to_string()],
            links: vec!["adr:docs/adr/0001.md".to_string()],
            occurrence_limit: Some(2),
            lifecycle: Lifecycle {
                created: Some("2026-05-27".to_string()),
                review_after: Some("2026-07-01".to_string()),
                expires: Some("2026-08-02".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                container: Some("parse".to_string()),
                callee: Some("unwrap".to_string()),
                macro_name: None,
                lint: None,
                symbol: Some("value.unwrap()".to_string()),
                receiver_fingerprint: None,
                target_fingerprint: None,
                normalized_snippet_hash: Some("fnv1a64:test".to_string()),
                line_hint: Some(12),
                glob: None,
            },
            last_seen: Some(LastSeen {
                line: 12,
                column: 9,
            }),
        };
        let mut identity = StructuralIdentity::new("rust", "method_call");
        identity.crate_name = Some("parser".to_string());
        identity.container = Some("parse".to_string());
        let finding = Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("crates\\parser\\src\\lib.rs"),
            span: Some(Span {
                line: 12,
                column: 9,
            }),
            identity,
            message: "unwrap call".to_string(),
        };

        let entry_json = render_allow_entry_json(&entry, "  ");
        let finding_json = render_explain_finding_json(&finding, "selected", "  ");

        assert!(entry_json.contains("\"id\": \"allow-json\""));
        assert!(entry_json.contains("\"path\": \"crates/parser/src/lib.rs\""));
        assert!(entry_json.contains("\"occurrence_limit\": 2"));
        assert!(entry_json.contains("\"normalized_snippet_hash\": \"fnv1a64:test\""));
        assert!(entry_json.contains("\"line\": 12"));
        assert!(finding_json.contains("\"status\": \"selected\""));
        assert!(finding_json.contains("\"path\": \"crates/parser/src/lib.rs\""));
        assert!(finding_json.contains("\"source_package\": \"parser\""));
        assert!(finding_json.contains("\"container\": \"parse\""));
    }

    #[test]
    fn add_json_renderer_records_entry_and_selected_finding() {
        let entry = AllowEntry {
            id: "allow-add-json".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src\\lib.rs")),
            glob: None,
            owner: "parser".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Parser validates input before unwrapping.".to_string(),
            evidence: vec!["test:parser_validates_input".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-05-27".to_string()),
                review_after: Some("2026-11-01".to_string()),
                expires: Some("2027-01-01".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                container: Some("parse_span".to_string()),
                callee: Some("unwrap".to_string()),
                macro_name: None,
                lint: None,
                symbol: Some("value.unwrap()".to_string()),
                receiver_fingerprint: None,
                target_fingerprint: None,
                normalized_snippet_hash: Some("fnv1a64:add".to_string()),
                line_hint: Some(42),
                glob: None,
            },
            last_seen: Some(LastSeen {
                line: 42,
                column: 13,
            }),
        };
        let mut identity = StructuralIdentity::new("rust", "method_call");
        identity.crate_name = Some("parser".to_string());
        identity.container = Some("parse_span".to_string());
        identity.callee = Some("unwrap".to_string());
        let finding = Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("src\\lib.rs"),
            span: Some(Span {
                line: 42,
                column: 13,
            }),
            identity,
            message: "unwrap call".to_string(),
        };

        let json = render_add_json(AddReport {
            inventory: InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(52),
            ),
            entry: &entry,
            selected_finding: &finding,
            policy_output: Some("policy/allow.proposed.toml"),
            force: true,
        });

        assert!(json.contains("\"schema_id\": \"cargo-allow.add.v1\""));
        assert!(json.contains("\"command\": \"add\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 52"));
        assert!(json.contains("\"policy_output\": \"policy/allow.proposed.toml\""));
        assert!(json.contains("\"force\": true"));
        assert!(json.contains("\"entry_id\": \"allow-add-json\""));
        assert!(json.contains("\"selected_finding\": \"src/lib.rs:42:13\""));
        assert!(json.contains("\"human_review_required\": true"));
        assert!(json.contains("\"id\": \"allow-add-json\""));
        assert!(json.contains("\"path\": \"src/lib.rs\""));
        assert!(json.contains("\"review_after\": \"2026-11-01\""));
        assert!(json.contains("\"expires\": \"2027-01-01\""));
        assert!(json.contains("\"evidence_count\": 1"));
        assert!(json.contains("\"source_package\": \"parser\""));
        assert!(json.contains("\"normalized_snippet_hash\": \"fnv1a64:add\""));
    }

    #[test]
    fn explain_json_renderer_records_context_and_current_status() {
        let entry = AllowEntry {
            id: "allow-explain-json".to_string(),
            kind: FindingKind::Unsafe,
            family: Some("unsafe_block".to_string()),
            path: Some(PathBuf::from("src\\ffi.rs")),
            glob: None,
            owner: "runtime".to_string(),
            classification: "ffi_boundary".to_string(),
            reason: "FFI pointer boundary requires unsafe.".to_string(),
            evidence: vec!["doc:docs/safety/ffi.md".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-05-27".to_string()),
                review_after: Some("2026-11-01".to_string()),
                expires: None,
            },
            selector: Selector {
                ast_kind: Some("unsafe_block".to_string()),
                container: Some("read_byte".to_string()),
                callee: None,
                macro_name: None,
                lint: None,
                symbol: None,
                receiver_fingerprint: None,
                target_fingerprint: None,
                normalized_snippet_hash: Some("fnv1a64:unsafe".to_string()),
                line_hint: Some(9),
                glob: None,
            },
            last_seen: Some(LastSeen { line: 9, column: 5 }),
        };
        let mut identity = StructuralIdentity::new("rust", "unsafe_block");
        identity.crate_name = Some("runtime".to_string());
        identity.container = Some("read_byte".to_string());
        let finding = Finding {
            kind: FindingKind::Unsafe,
            family: Some("unsafe_block".to_string()),
            path: PathBuf::from("src\\ffi.rs"),
            span: Some(Span { line: 9, column: 5 }),
            identity,
            message: "unsafe block".to_string(),
        };
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::EvidenceMissing,
            allow_id: Some("allow-explain-json".to_string()),
            finding_index: Some(0),
            message: "unsafe entry has missing evidence".to_string(),
            score: 9,
        }];
        let evidence_references = vec![EvidenceReference {
            raw: "doc:docs/safety/ffi.md",
            prefix: Some("doc"),
            target: Some("docs/safety/ffi.md"),
            status: "missing",
            message: "local evidence file is missing",
        }];
        let suggested_actions = vec!["add missing evidence".to_string()];
        let proof_commands = vec!["cargo-allow check --kind unsafe".to_string()];

        let json = render_explain_json(ExplainReport {
            inventory: InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(76),
            ),
            entry: &entry,
            current_findings: &[finding],
            match_outcomes: &outcomes,
            evidence_references: &evidence_references,
            suggested_actions: &suggested_actions,
            proof_commands: &proof_commands,
        });

        assert!(json.contains("\"schema_id\": \"cargo-allow.explain.v1\""));
        assert!(json.contains("\"command\": \"explain\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 76"));
        assert!(json.contains("\"id\": \"allow-explain-json\""));
        assert!(json.contains("\"current_status\": \"evidence_missing\""));
        assert!(json.contains("\"current_matches\": 1"));
        assert!(json.contains("\"match_outcomes\": 1"));
        assert!(json.contains("\"raw\": \"doc:docs/safety/ffi.md\""));
        assert!(json.contains("\"target\": \"docs/safety/ffi.md\""));
        assert!(json.contains("\"status\": \"missing\""));
        assert!(json.contains("\"path\": \"src/ffi.rs\""));
        assert!(json.contains("\"source_package\": \"runtime\""));
        assert!(json.contains("\"score\": 9"));
        assert!(json.contains("\"add missing evidence\""));
        assert!(json.contains("\"cargo-allow check --kind unsafe\""));
    }

    #[test]
    fn diff_json_renderer_appends_posture_extension() {
        let finding_changes = vec![DiffFindingChange {
            change: "new",
            key: "panic|unwrap|src/lib.rs",
            kind: "panic",
            family: Some("unwrap"),
            path: "src/lib.rs",
        }];
        let policy_changes = vec![DiffPolicyChange {
            severity: "fail",
            allow_id: "allow-0001",
            kind: "scope_broadened",
            message: "allow-0001 selector scope broadened",
        }];

        let rendered = render_diff_json_with_posture(
            "{\n  \"schema_id\": \"cargo-allow.report.v1\"\n}",
            DiffReport {
                net_posture: "worse",
                reviewer_action: "block until fixed",
                summary: DiffPostureSummary {
                    current_failures: 1,
                    new_findings: 1,
                    removed_findings: 0,
                    policy_failures: 1,
                    policy_review_items: 0,
                    policy_improvements: 0,
                },
                finding_changes: &finding_changes,
                policy_changes: &policy_changes,
            },
        );
        assert!(rendered.is_some());
        let Some(json) = rendered else {
            return;
        };

        assert!(json.contains("\"diff\""));
        assert!(json.contains("\"net_posture\": \"worse\""));
        assert!(json.contains("\"reviewer_action\": \"block until fixed\""));
        assert!(json.contains("\"current_failures\": 1"));
        assert!(json.contains("\"new_findings\": 1"));
        assert!(json.contains("\"policy_failures\": 1"));
        assert!(json.contains("\"change\": \"new\""));
        assert!(json.contains("\"family\": \"unwrap\""));
        assert!(json.contains("\"severity\": \"fail\""));
        assert!(json.contains("\"kind\": \"scope_broadened\""));
        assert!(json.ends_with("}\n"));
        assert!(
            render_diff_json_with_posture(
                "not json",
                DiffReport {
                    net_posture: "unchanged",
                    reviewer_action: "none",
                    summary: DiffPostureSummary {
                        current_failures: 0,
                        new_findings: 0,
                        removed_findings: 0,
                        policy_failures: 0,
                        policy_review_items: 0,
                        policy_improvements: 0,
                    },
                    finding_changes: &[],
                    policy_changes: &[],
                },
            )
            .is_none()
        );
    }

    #[test]
    fn prune_json_renderer_records_mode_context_and_candidates() {
        let candidates = vec![PruneCandidate {
            id: "allow-stale",
            kind: "panic",
            family: Some("unwrap"),
            owner: "parser",
            classification: "baseline_debt",
            scope: "crates/parser/src/lib.rs",
            reason: "stale baseline entry",
        }];

        let json = render_prune_json(
            &candidates,
            PruneModeContext {
                explicit_dry_run: true,
                write_requested: false,
                written_path: None,
            },
            InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(49),
            ),
        );

        assert!(json.contains("\"schema_id\": \"cargo-allow.prune.v1\""));
        assert!(json.contains("\"command\": \"prune\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 49"));
        assert!(json.contains("\"dry_run\": true"));
        assert!(json.contains("\"write_requested\": false"));
        assert!(json.contains("\"explicit_dry_run\": true"));
        assert!(json.contains("\"written_path\": null"));
        assert!(json.contains("\"stale_entries\": 1"));
        assert!(json.contains("\"id\": \"allow-stale\""));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"family\": \"unwrap\""));
    }

    #[test]
    fn list_json_renderer_records_filters_context_and_rows() {
        let rows = vec![ListRow {
            id: "allow-json",
            status: "baseline_debt",
            matches: 1,
            kind: "panic",
            family: Some("unwrap"),
            owner: "parser",
            classification: "baseline_debt",
            scope: "crates/parser/src/lib.rs",
            source_package: Some("parser"),
            evidence_count: 2,
            review_after: Some("2026-07-01"),
            expires: None,
            reason: "generated baseline",
        }];

        let json = render_list_json(
            &rows,
            ListFilters {
                kind: Some("panic"),
                family: Some("unwrap"),
                owner: Some("parser"),
                baseline_debt: true,
                ..ListFilters::default()
            },
            InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(46),
            ),
        );

        assert!(json.contains("\"schema_id\": \"cargo-allow.list.v1\""));
        assert!(json.contains("\"command\": \"list\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 46"));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"family\": \"unwrap\""));
        assert!(json.contains("\"owner\": \"parser\""));
        assert!(json.contains("\"baseline_debt\": true"));
        assert!(json.contains("\"allow_entries\": 1"));
        assert!(json.contains("\"id\": \"allow-json\""));
        assert!(json.contains("\"source_package\": \"parser\""));
        assert!(json.contains("\"review_after\": \"2026-07-01\""));
        assert!(json.contains("\"expires\": null"));
    }

    #[test]
    fn worklist_json_renderer_records_filters_summary_and_items() {
        let suggested_actions = vec!["review stale allow".to_string()];
        let proof_commands = vec!["cargo-allow check --mode no-new".to_string()];
        let items = vec![WorklistItem {
            id: "work-0001",
            kind: "stale_allow",
            exception_kind: Some("panic"),
            family: Some("unwrap"),
            owner: Some("parser"),
            classification: Some("baseline_debt"),
            reason: Some("generated baseline"),
            created: Some("2026-05-27"),
            review_after: Some("2026-07-01"),
            expires: Some("2026-08-02"),
            evidence_count: Some(1),
            risk: "high",
            difficulty: "small",
            status: "stale",
            allow_id: Some("allow-0001"),
            finding_index: None,
            path: Some("crates/parser/src/lib.rs"),
            source_package: Some("parser"),
            message: "stale allow",
            suggested_actions: &suggested_actions,
            proof_commands: &proof_commands,
        }];

        let json = render_worklist_json(
            &items,
            WorklistFilters {
                kind: Some("panic"),
                item_kind: Some("stale_allow"),
                risk: Some("high"),
                baseline_debt: true,
                missing_evidence: true,
                ..WorklistFilters::default()
            },
            InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(47),
            ),
        );

        assert!(json.contains("\"schema_id\": \"cargo-allow.worklist.v1\""));
        assert!(json.contains("\"command\": \"worklist\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 47"));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"item_kind\": \"stale_allow\""));
        assert!(json.contains("\"baseline_debt\": true"));
        assert!(json.contains("\"missing_evidence\": true"));
        assert!(json.contains("\"work_items\": 1"));
        assert!(json.contains("\"high\": 1"));
        assert!(json.contains("\"small_difficulty\": 1"));
        assert!(json.contains("\"id\": \"work-0001\""));
        assert!(json.contains("\"exception_kind\": \"panic\""));
        assert!(json.contains("\"evidence_count\": 1"));
        assert!(json.contains("\"finding_index\": null"));
        assert!(json.contains("\"source_package\": \"parser\""));
        assert!(json.contains("\"suggested_actions\": [\"review stale allow\"]"));
        assert!(json.contains("\"proof_commands\": [\"cargo-allow check --mode no-new\"]"));
    }

    #[test]
    fn doctor_json_renderer_records_root_config_and_inventory() {
        let json = render_doctor_json(DoctorReport {
            source_tree_root: "H:/Code/Rust/cargo-allow",
            root_discovery: "nearest_git_root",
            config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
            inventory_source: "git_tracked",
            files_scanned: 50,
        });

        assert!(json.contains("\"schema_id\": \"cargo-allow.doctor.v1\""));
        assert!(json.contains("\"command\": \"doctor\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"discovery\": \"nearest_git_root\""));
        assert!(json.contains("\"found\": true"));
        assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow/policy/allow.toml\""));
        assert!(json.contains("\"scanner\": \"source_syntax\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 50"));
    }

    #[test]
    fn propose_json_renderer_records_options_summary_and_defaults() {
        let json = render_propose_json(ProposeReport {
            inventory: InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(76),
            ),
            kind: Some("panic"),
            expires: "2026-08-02",
            policy_output: Some("target/cargo-allow/proposed.toml"),
            force: true,
            findings_scanned: 54,
            baseline_debt_entries_proposed: 2,
        });

        assert!(json.contains("\"schema_id\": \"cargo-allow.propose.v1\""));
        assert!(json.contains("\"command\": \"propose\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 76"));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"expires\": \"2026-08-02\""));
        assert!(json.contains("\"policy_output\": \"target/cargo-allow/proposed.toml\""));
        assert!(json.contains("\"force\": true"));
        assert!(json.contains("\"findings_scanned\": 54"));
        assert!(json.contains("\"baseline_debt_entries_proposed\": 2"));
        assert!(json.contains("\"owner\": \"unowned\""));
        assert!(json.contains("\"classification\": \"baseline_debt\""));
    }

    #[test]
    fn migrate_json_renderer_records_io_summary_and_notes() {
        let json = render_migrate_json(MigrateReport {
            inventory: InventoryContext::new(
                "source_tree",
                "policy_migration",
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(76),
            ),
            input_kind: "repo_policy",
            input_path: "policy",
            output_path: "policy/allow.toml",
            force: true,
            allow_entries: 12,
            baseline_debt: 5,
            unsafe_entries: 2,
            entries_with_evidence: 3,
            notes: "migration notes",
        });

        assert!(json.contains("\"schema_id\": \"cargo-allow.migrate.v1\""));
        assert!(json.contains("\"command\": \"migrate\""));
        assert!(json.contains("\"scanner\": \"policy_migration\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 76"));
        assert!(json.contains("\"kind\": \"repo_policy\""));
        assert!(json.contains("\"path\": \"policy\""));
        assert!(json.contains("\"path\": \"policy/allow.toml\""));
        assert!(json.contains("\"force\": true"));
        assert!(json.contains("\"allow_entries\": 12"));
        assert!(json.contains("\"baseline_debt\": 5"));
        assert!(json.contains("\"unsafe_entries\": 2"));
        assert!(json.contains("\"entries_with_evidence\": 3"));
        assert!(json.contains("\"notes\": \"migration notes\""));
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
        let add_schema = include_str!("../../../docs/schemas/add.schema.json");
        let migrate_schema = include_str!("../../../docs/schemas/migrate.schema.json");
        assert!(report_schema.contains(REPORT_SCHEMA_ID));
        assert!(receipt_schema.contains(RECEIPT_SCHEMA_ID));
        assert!(worklist_schema.contains(WORKLIST_SCHEMA_ID));
        assert!(list_schema.contains(LIST_SCHEMA_ID));
        assert!(explain_schema.contains(EXPLAIN_SCHEMA_ID));
        assert!(prune_schema.contains(PRUNE_SCHEMA_ID));
        assert!(doctor_schema.contains(DOCTOR_SCHEMA_ID));
        assert!(propose_schema.contains(PROPOSE_SCHEMA_ID));
        assert!(add_schema.contains(ADD_SCHEMA_ID));
        assert!(migrate_schema.contains(MIGRATE_SCHEMA_ID));
        assert!(report_schema.contains("\"files_scanned\""));
        assert!(receipt_schema.contains("\"files_scanned\""));
        assert!(list_schema.contains("\"files_scanned\""));
        assert!(explain_schema.contains("\"files_scanned\""));
        assert!(prune_schema.contains("\"files_scanned\""));
        assert!(doctor_schema.contains("\"files_scanned\""));
        assert!(propose_schema.contains("\"files_scanned\""));
        assert!(add_schema.contains("\"files_scanned\""));
        assert!(migrate_schema.contains("\"files_scanned\""));
        assert!(report_schema.contains("\"root\""));
        assert!(receipt_schema.contains("\"root\""));
        assert!(list_schema.contains("\"root\""));
        assert!(explain_schema.contains("\"root\""));
        assert!(prune_schema.contains("\"root\""));
        assert!(doctor_schema.contains("\"root\""));
        assert!(propose_schema.contains("\"root\""));
        assert!(add_schema.contains("\"root\""));
        assert!(migrate_schema.contains("\"root\""));
        assert!(report_schema.contains("\"source_package\""));
        assert!(list_schema.contains("\"source_package\""));
        assert!(explain_schema.contains("\"source_package\""));
        assert!(add_schema.contains("\"source_package\""));
        assert!(report_schema.contains("\"scanner_limitation\""));
        assert!(receipt_schema.contains("\"scanner_limitation\""));
        assert!(list_schema.contains("\"scanner_limitation\""));
        assert!(explain_schema.contains("\"scanner_limitation\""));
        assert!(prune_schema.contains("\"scanner_limitation\""));
        assert!(doctor_schema.contains("\"scanner_limitation\""));
        assert!(propose_schema.contains("\"scanner_limitation\""));
        assert!(add_schema.contains("\"scanner_limitation\""));
        assert!(migrate_schema.contains("\"scanner_limitation\""));
        assert!(report_schema.contains("\"repository_code_not_executed\""));
        assert!(receipt_schema.contains("\"repository_code_not_executed\""));
        assert!(list_schema.contains("\"repository_code_not_executed\""));
        assert!(explain_schema.contains("\"repository_code_not_executed\""));
        assert!(prune_schema.contains("\"repository_code_not_executed\""));
        assert!(doctor_schema.contains("\"repository_code_not_executed\""));
        assert!(propose_schema.contains("\"repository_code_not_executed\""));
        assert!(add_schema.contains("\"repository_code_not_executed\""));
        assert!(migrate_schema.contains("\"repository_code_not_executed\""));
        for limitation in SCANNER_LIMITATIONS {
            assert!(report_schema.contains(limitation));
            assert!(receipt_schema.contains(limitation));
            assert!(worklist_schema.contains(limitation));
            assert!(list_schema.contains(limitation));
            assert!(explain_schema.contains(limitation));
            assert!(prune_schema.contains(limitation));
            assert!(doctor_schema.contains(limitation));
            assert!(propose_schema.contains(limitation));
            assert!(add_schema.contains(limitation));
            assert!(migrate_schema.contains(limitation));
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
            assert!(add_schema.contains(claim));
            assert!(migrate_schema.contains(claim));
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
