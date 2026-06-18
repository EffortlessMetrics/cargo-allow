//! Human and machine artifact rendering for cargo-allow.
//!
//! This crate renders reports, receipts, PR summaries, explanations, lists,
//! worklists, migration summaries, SARIF, and HTML while preserving the
//! source-tree claim boundary. Renderers describe what cargo-allow scanned and
//! what it did not execute; they do not perform scanning or validation.

mod add;
#[cfg(test)]
mod add_tests;
mod allow_entry_json;
#[cfg(test)]
mod allow_entry_json_tests;
mod artifacts;
mod audit_remediation;
#[cfg(test)]
mod audit_remediation_tests;
mod contracts;
mod diff;
mod diff_finding_detail;
#[cfg(test)]
mod diff_finding_detail_tests;
mod diff_human;
mod diff_json;
mod diff_markdown;
mod diff_policy_detail;
mod diff_posture;
mod doctor;
#[cfg(test)]
mod doctor_tests;
mod evidence_reference_human;
mod evidence_repair;
#[cfg(test)]
mod evidence_repair_tests;
mod explain;
mod explain_common;
#[cfg(test)]
mod explain_common_tests;
mod explain_human;
mod explain_json;
#[cfg(test)]
mod explain_tests;
mod html;
#[cfg(test)]
mod html_tests;
mod json;
#[cfg(test)]
mod json_tests;
mod list;
#[cfg(test)]
mod list_tests;
mod migrate;
mod migrate_closeout;
mod migrate_closeout_queues;
#[cfg(test)]
mod migrate_tests;
mod non_rust;
mod path_text;
#[cfg(test)]
mod path_text_tests;
mod propose;
#[cfg(test)]
mod propose_tests;
mod prune;
#[cfg(test)]
mod prune_tests;
mod receipt;
#[cfg(test)]
mod receipt_tests;
mod report_json;
mod report_text;
mod sarif;
#[cfg(test)]
mod sarif_tests;
mod source_inventory;
mod summary;
mod text;
mod worklist;
mod worklist_human;
mod worklist_json;
mod worklist_summary;
#[cfg(test)]
mod worklist_summary_tests;

pub use add::{render_add_human, render_add_json};
pub use allow_entry_json::{render_allow_entry_json, render_last_seen_json, render_selector_json};
pub use artifacts::{
    AddReport, DiffEvidenceChange, DiffExceptionIdentityChange, DiffFindingChange,
    DiffLifecycleChange, DiffMetadataChange, DiffOccurrenceLimitChange, DiffPolicyChange,
    DiffPolicyStatusChange, DiffPostureSummary, DiffReport, DiffRequirementChange, DiffScopeChange,
    DiffSelectorIdentityChange, DiffSelectorPrecisionChange, DoctorReport, EvidenceReference,
    ExplainReport, ListFilters, ListRow, MigrateReport, ProposeReport, PruneCandidate,
    PruneModeContext, WorklistFilters, WorklistItem,
};
pub use contracts::{
    ADD_SCHEMA_ID, ADD_SCHEMA_VERSION, ARTIFACT_CONTRACTS, ARTIFACT_STATUS_ERROR,
    ARTIFACT_STATUS_FAILED, ARTIFACT_STATUS_PASSED, ARTIFACT_STATUSES, ArtifactContract,
    CLAIM_BOUNDARY, CLAIM_BOUNDARY_TEXT, DOCTOR_SCHEMA_ID, DOCTOR_SCHEMA_VERSION,
    EXPLAIN_SCHEMA_ID, EXPLAIN_SCHEMA_VERSION, INVENTORY_SCANNER_POLICY_MIGRATION,
    INVENTORY_SCANNER_SOURCE_SYNTAX, INVENTORY_SCANNER_SOURCE_TREE_GRAPH,
    INVENTORY_SCOPE_SOURCE_TREE, INVENTORY_SOURCE_UNKNOWN, InventoryContext, LIST_SCHEMA_ID,
    LIST_SCHEMA_VERSION, MIGRATE_SCHEMA_ID, MIGRATE_SCHEMA_VERSION, PROPOSE_SCHEMA_ID,
    PROPOSE_SCHEMA_VERSION, PRUNE_SCHEMA_ID, PRUNE_SCHEMA_VERSION, RECEIPT_COMMAND_CHECK,
    RECEIPT_ENFORCEMENT_ADVISORY, RECEIPT_ENFORCEMENT_ENFORCING, RECEIPT_SCHEMA_ID,
    RECEIPT_SCHEMA_VERSION, RECEIPT_STATUSES, REPORT_COMMAND_AUDIT, REPORT_COMMAND_CHECK,
    REPORT_COMMAND_DIFF, REPORT_COMMANDS, REPORT_SCHEMA_ID, REPORT_SCHEMA_VERSION, ReportContext,
    SCANNER_LIMITATIONS, SPEC_SYSTEM_CLAIM_BOUNDARY, SPEC_SYSTEM_SCANNER_LIMITATIONS,
    SPEC_SYSTEM_SCHEMA_ID, SPEC_SYSTEM_SCHEMA_VERSION, WORKLIST_SCHEMA_ID, WORKLIST_SCHEMA_VERSION,
    artifact_contract_for_schema_id, claim_boundary_for_schema_id,
    scanner_limitations_for_schema_id,
};
pub use diff::{
    DiffNetPosture, diff_net_posture, diff_posture_summary, insert_markdown_pr_summary,
    render_diff_finding_changes_human, render_diff_finding_changes_markdown,
    render_diff_json_with_posture, render_diff_policy_changes_human,
    render_diff_policy_changes_markdown, render_diff_posture_summary_human,
    render_diff_posture_summary_human_with_evidence_health,
    render_diff_posture_summary_human_with_evidence_health_counts, render_diff_pr_summary_markdown,
    render_diff_pr_summary_markdown_with_evidence_health,
    render_diff_pr_summary_markdown_with_evidence_health_counts,
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
pub use migrate_closeout::{
    MigrateCloseoutInput, MigrateLegacySource, migrate_closeout_from_input,
};
pub use path_text::source_tree_path_text;
pub use propose::{render_propose_human, render_propose_json};
pub use prune::{render_prune_human, render_prune_human_with_context, render_prune_json};
pub use receipt::{
    render_error_receipt, render_receipt, render_receipt_with_context,
    render_receipt_with_context_and_inventory,
};
pub use report_json::{render_json, render_json_with_context, render_json_with_context_and_diff};
pub use report_text::{
    render_human, render_human_with_context, render_markdown, render_markdown_with_context,
};
pub use sarif::{render_sarif, render_sarif_with_context};
pub use summary::{
    Summary, matched_policy_missing_evidence_entries, policy_baseline_debt_entries,
    policy_missing_evidence_entries,
};
pub use worklist::{render_worklist_human, render_worklist_json};

pub(crate) use non_rust::{FilePosture, non_rust_file_rows};
pub(crate) use source_inventory::{
    render_source_inventory_html, render_source_inventory_human, render_source_inventory_json,
    render_source_inventory_markdown,
};
pub(crate) use summary::{
    AUDIT_REVIEW_QUEUE_STATUSES, ReviewSignals, STATUS_COUNT_ORDER, audit_review_queue,
    baseline_debt_count, broken_evidence_link_count, policy_missing_evidence_count,
    render_advisory_count_fields, render_count_fields_with_policy_context,
    weak_evidence_reference_count,
};

#[cfg(test)]
mod diff_human_tests;
#[cfg(test)]
mod diff_json_detail_tests;
#[cfg(test)]
mod diff_json_tests;
#[cfg(test)]
mod diff_markdown_tests;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod text_tests;
#[cfg(test)]
mod worklist_tests;
