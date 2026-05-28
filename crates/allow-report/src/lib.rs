use allow_core::{MatchOutcome, MatchStatus};
use std::collections::BTreeMap;

mod add;
mod allow_entry_json;
mod artifacts;
mod contracts;
mod diff;
mod doctor;
mod explain;
mod html;
mod json;
mod list;
mod migrate;
mod non_rust;
mod propose;
mod prune;
mod receipt;
mod report_json;
mod report_text;
mod sarif;
mod text;
mod worklist;

pub use add::{render_add_human, render_add_json};
pub use allow_entry_json::{render_allow_entry_json, render_last_seen_json, render_selector_json};
pub use artifacts::{
    AddReport, DiffFindingChange, DiffPolicyChange, DiffPostureSummary, DiffReport, DoctorReport,
    EvidenceReference, ExplainReport, ListFilters, ListRow, MigrateReport, ProposeReport,
    PruneCandidate, PruneModeContext, WorklistFilters, WorklistItem,
};
pub use contracts::{
    ADD_SCHEMA_ID, ADD_SCHEMA_VERSION, CLAIM_BOUNDARY, CLAIM_BOUNDARY_TEXT, DOCTOR_SCHEMA_ID,
    DOCTOR_SCHEMA_VERSION, EXPLAIN_SCHEMA_ID, EXPLAIN_SCHEMA_VERSION, InventoryContext,
    LIST_SCHEMA_ID, LIST_SCHEMA_VERSION, MIGRATE_SCHEMA_ID, MIGRATE_SCHEMA_VERSION,
    PROPOSE_SCHEMA_ID, PROPOSE_SCHEMA_VERSION, PRUNE_SCHEMA_ID, PRUNE_SCHEMA_VERSION,
    RECEIPT_SCHEMA_ID, RECEIPT_SCHEMA_VERSION, REPORT_SCHEMA_ID, REPORT_SCHEMA_VERSION,
    ReportContext, SCANNER_LIMITATIONS, WORKLIST_SCHEMA_ID, WORKLIST_SCHEMA_VERSION,
};
pub use diff::{
    DiffNetPosture, diff_net_posture, diff_posture_summary, insert_markdown_pr_summary,
    render_diff_finding_changes_human, render_diff_finding_changes_markdown,
    render_diff_json_with_posture, render_diff_policy_changes_human,
    render_diff_policy_changes_markdown, render_diff_pr_summary_markdown,
};
pub use doctor::{render_doctor_human, render_doctor_json};
pub(crate) use explain::finding_location_text;
pub use explain::{render_explain_finding_json, render_explain_human, render_explain_json};
pub use html::{render_html, render_html_with_context};
pub use json::{
    render_claim_boundary_json, render_inventory_json, render_scanner_limitations_json,
};
pub use list::{render_list_human, render_list_json};
pub use migrate::{render_migrate_human, render_migrate_json};
pub use propose::{render_propose_human, render_propose_json};
pub use prune::{render_prune_human, render_prune_json};
pub use receipt::{render_receipt, render_receipt_with_context};
pub use report_json::{render_json, render_json_with_context};
pub use report_text::{
    render_human, render_human_with_context, render_markdown, render_markdown_with_context,
};
pub use sarif::{render_sarif, render_sarif_with_context};
pub use worklist::{render_worklist_human, render_worklist_json};

pub(crate) use non_rust::{FilePosture, non_rust_file_rows};

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

pub(crate) fn render_counts_fields(summary: &Summary, indent: &str) -> String {
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

pub(crate) fn review_item_count_with_baseline(summary: &Summary, baseline_debt: usize) -> usize {
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

pub(crate) fn baseline_debt_count(summary: &Summary, context: ReportContext<'_>) -> usize {
    context
        .baseline_debt_entries
        .unwrap_or_else(|| summary.count(MatchStatus::BaselineDebt))
}

pub(crate) fn audit_review_queue(outcomes: &[MatchOutcome]) -> Vec<&MatchOutcome> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .take(20)
        .collect()
}

#[cfg(test)]
mod tests;
