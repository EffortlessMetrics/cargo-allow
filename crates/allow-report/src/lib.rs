//! Human and machine artifact rendering for cargo-allow.
//!
//! This crate renders reports, receipts, PR summaries, explanations, lists,
//! worklists, migration summaries, SARIF, and HTML while preserving the
//! source-tree claim boundary. Renderers describe what cargo-allow scanned and
//! what it did not execute; they do not perform scanning or validation.

mod add;
mod add_finding_plan;
#[cfg(test)]
mod add_finding_plan_tests;
mod add_plan_application;
#[cfg(test)]
mod add_plan_application_tests;
#[cfg(test)]
mod add_tests;
#[cfg(test)]
mod adoption_plan_tests;
mod advisory_class;
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
mod diff_movement;
mod diff_policy_detail;
mod diff_posture;
#[cfg(test)]
mod diff_row_test_support;
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
mod ledger_posture;
mod list;
#[cfg(test)]
mod list_tests;
mod migrate;
mod migrate_closeout;
mod migrate_closeout_queues;
#[cfg(test)]
mod migrate_tests;
mod mutation_receipt;
#[cfg(test)]
mod mutation_receipt_tests;
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
mod read_model;
mod receipt;
#[cfg(test)]
mod receipt_tests;
mod refresh;
#[cfg(test)]
mod refresh_tests;
mod report_json;
mod report_text;
mod sarif;
#[cfg(test)]
mod sarif_tests;
mod source_inventory;
mod style;
mod summary;
mod text;
mod why;
mod why_json;
#[cfg(test)]
mod why_tests;
mod worklist;
mod worklist_human;
mod worklist_json;
mod worklist_summary;
#[cfg(test)]
mod worklist_summary_tests;

pub use add::{render_add_human, render_add_human_styled, render_add_json};
pub use add_finding_plan::{
    render_add_finding_plan_json, render_add_finding_plan_json_with_result_class,
};
pub use add_plan_application::render_add_plan_application_json;
pub use advisory_class::{ADVISORY_DENY_FIELD_NAMES, AdvisoryClass, advisory_count_for_deny_field};
pub use allow_entry_json::{render_allow_entry_json, render_last_seen_json, render_selector_json};
pub use artifacts::{
    AddFindingPlanCandidate, AddFindingPlanFinding, AddFindingPlanOutcome, AddFindingPlanPolicy,
    AddFindingPlanProofPlan, AddFindingPlanRepository, AddFindingPlanV1, AddPlanApplicationV1,
    AddReport, AdoptionAction, AdoptionActionKind, AdoptionFacts, AdoptionInventoryFacts,
    AdoptionPolicyFacts, BootstrapDisposition, ConfigProvenanceSummary, ConfiguredLedgerSummary,
    CoreAdoptionPlanV1, DiffEvidenceChange, DiffExceptionIdentityChange, DiffFindingChange,
    DiffLedgerMovementSummary, DiffLifecycleChange, DiffMetadataChange, DiffMovementCounts,
    DiffOccurrenceLimitChange, DiffPolicyChange, DiffPolicyStatusChange, DiffPostureDeltaCounts,
    DiffPostureSummary, DiffReport, DiffRequirementChange, DiffScopeChange,
    DiffSelectorIdentityChange, DiffSelectorPrecisionChange, DoctorReport, EvaluationContext,
    EvaluationResultClass, EvidenceReference, ExplainReport, FederationDiagnosticSummary,
    FederationDivergenceKindCount, FederationDivergenceRecordSummary, FederationDivergenceSummary,
    FederationReportContext, FileFamilyConflictSummary, FileFamilyRuleSummary,
    InventoryCompleteness, InventoryMode, LedgerContributorSummary, ListColumn, ListFilters,
    ListRow, MigrateReport, PolicyState, ProposeReport, PruneCandidate, PruneModeContext,
    RELEASE_MANIFEST_V2_SCHEMA_ID, RELEASE_MANIFEST_V2_SCHEMA_VERSION, RefreshModeContext,
    RefreshReport, ReleaseChannelV1, ReleaseIdentityErrorV1, ReleaseIdentityV1,
    ReleaseManifestAuthenticationV2, ReleaseManifestEnvelopeV2, ReleaseManifestOperationV2,
    ReleaseManifestPackageRowV2, ReleaseManifestPayloadV2, ReleaseManifestPublicationPostureV2,
    ReleaseManifestResultV2, ReleaseManifestSupportPostureV2, ReleaseManifestV2Validation,
    ReleaseVersionV1, WhyCandidateEntry, WhyProofPlan, WhyReport, WhyTargetScan,
    WhyTargetScanReport, WorklistFilters, WorklistItem, WritePosture, recommend_core_adoption_plan,
    render_release_manifest_v2_envelope, render_release_manifest_v2_envelope_bytes,
    render_release_manifest_v2_payload, render_release_manifest_v2_payload_bytes,
    validate_release_manifest_v2,
};

pub use artifacts::{
    LandedEffectStatusV1, LandedEffectV1, NextFrontierV1, PostMergeReconciliationRequestV1,
    PostMergeReconciliationResultV1, ReconciliationDispositionV1,
};
pub use contracts::{
    ADD_FINDING_PLAN_CLAIM_BOUNDARY, ADD_FINDING_PLAN_SCHEMA_ID, ADD_FINDING_PLAN_SCHEMA_VERSION,
    ADD_PLAN_APPLICATION_CLAIM_BOUNDARY, ADD_PLAN_APPLICATION_SCHEMA_ID,
    ADD_PLAN_APPLICATION_SCHEMA_VERSION, ADD_SCHEMA_ID, ADD_SCHEMA_VERSION, ARTIFACT_CONTRACTS,
    ARTIFACT_STATUS_ERROR, ARTIFACT_STATUS_FAILED, ARTIFACT_STATUS_PASSED, ARTIFACT_STATUSES,
    ArtifactContract, CLAIM_BOUNDARY, CLAIM_BOUNDARY_TEXT, CORE_ADOPTION_PLAN_SCHEMA_ID,
    CORE_ADOPTION_PLAN_SCHEMA_VERSION, DOCTOR_SCHEMA_ID, DOCTOR_SCHEMA_VERSION,
    DiffAnalysisContext, EXPLAIN_SCHEMA_ID, EXPLAIN_SCHEMA_VERSION,
    INVENTORY_SCANNER_POLICY_MIGRATION, INVENTORY_SCANNER_SOURCE_SYNTAX,
    INVENTORY_SCANNER_SOURCE_TREE_GRAPH, INVENTORY_SCOPE_SOURCE_TREE, INVENTORY_SOURCE_UNKNOWN,
    InventoryContext, LIST_SCHEMA_ID, LIST_SCHEMA_VERSION, MIGRATE_SCHEMA_ID,
    MIGRATE_SCHEMA_VERSION, PROPOSE_SCHEMA_ID, PROPOSE_SCHEMA_VERSION, PRUNE_SCHEMA_ID,
    PRUNE_SCHEMA_VERSION, RECEIPT_COMMAND_CHECK, RECEIPT_COMMAND_DIFF, RECEIPT_COMMANDS,
    RECEIPT_ENFORCEMENT_ADVISORY, RECEIPT_ENFORCEMENT_ENFORCING, RECEIPT_SCHEMA_ID,
    RECEIPT_SCHEMA_VERSION, RECEIPT_STATUSES, REFRESH_SCHEMA_ID, REFRESH_SCHEMA_VERSION,
    REPORT_COMMAND_AUDIT, REPORT_COMMAND_CHECK, REPORT_COMMAND_DIFF, REPORT_COMMANDS,
    REPORT_SCHEMA_ID, REPORT_SCHEMA_VERSION, ReportContext, SCANNER_LIMITATIONS,
    SPEC_SYSTEM_CLAIM_BOUNDARY, SPEC_SYSTEM_SCANNER_LIMITATIONS, SPEC_SYSTEM_SCHEMA_ID,
    SPEC_SYSTEM_SCHEMA_VERSION, WHY_SCHEMA_ID, WHY_SCHEMA_VERSION, WORKLIST_SCHEMA_ID,
    WORKLIST_SCHEMA_VERSION, artifact_contract_for_schema_id, claim_boundary_for_schema_id,
    is_quiet, scanner_limitations_for_schema_id,
};
pub use diff::{
    DiffNetPosture, diff_net_posture, diff_posture_summary, insert_markdown_pr_summary,
    render_diff_analysis_human, render_diff_analysis_markdown, render_diff_finding_changes_human,
    render_diff_finding_changes_human_styled, render_diff_finding_changes_markdown,
    render_diff_json_with_posture, render_diff_policy_changes_human,
    render_diff_policy_changes_human_styled, render_diff_policy_changes_markdown,
    render_diff_posture_summary_human, render_diff_posture_summary_human_styled,
    render_diff_posture_summary_human_with_evidence_health,
    render_diff_posture_summary_human_with_evidence_health_counts,
    render_diff_posture_summary_human_with_evidence_health_counts_styled,
    render_diff_posture_summary_human_with_evidence_health_styled, render_diff_pr_summary_markdown,
    render_diff_pr_summary_markdown_with_evidence_health,
    render_diff_pr_summary_markdown_with_evidence_health_counts,
};
pub use doctor::{render_doctor_human, render_doctor_human_styled, render_doctor_json};
pub(crate) use explain::finding_location_text;
pub use explain::{
    render_explain_finding_json, render_explain_human, render_explain_human_styled,
    render_explain_json,
};
pub use html::{render_html, render_html_with_context};
pub use json::{
    render_claim_boundary_json, render_inventory_json, render_scanner_limitations_json,
};
pub use ledger_posture::{
    FINDING_CHANGE_LABELS, LedgerPosture, MOVEMENT_PROJECTION_LABELS, NET_POSTURE_LABELS,
    NetPosture, POSTURE_DELTA_FIELD_NAMES, PostureDelta, PresenceMovement,
    finding_change_label_for, parse_finding_change_label,
};
pub use list::{
    render_list_human, render_list_human_columns, render_list_human_columns_styled,
    render_list_human_concise, render_list_human_concise_styled,
    render_list_human_concise_styled_with_width, render_list_json,
};
pub use migrate::{render_migrate_human, render_migrate_human_styled, render_migrate_json};
pub use migrate_closeout::{
    BASELINE_DEBT_ITEM_KIND, MISSING_EVIDENCE_ITEM_KIND, MigrateBaselineDebtProjection,
    MigrateCloseoutInput, MigrateLegacySource, NO_NEW_GATE_ITEM_KIND, NO_NEW_GATE_SIGNAL,
    migrate_closeout_from_input,
};
pub use mutation_receipt::{
    MUTATION_RECEIPT_CLAIM_BOUNDARY, MUTATION_RECEIPT_SCHEMA_ID, MUTATION_RECEIPT_SCHEMA_VERSION,
    MutationReceipt, render_mutation_receipt_json,
};
pub use path_text::source_tree_path_text;
pub use propose::{render_propose_human, render_propose_human_styled, render_propose_json};
pub use prune::{
    render_prune_human, render_prune_human_with_context, render_prune_human_with_context_styled,
    render_prune_json,
};
pub use read_model::{
    LedgerReadState, ledger_project_outcomes, ledger_read_state, ledger_read_state_for_outcomes,
    ledger_read_statuses,
};
pub use receipt::{
    render_error_receipt, render_receipt, render_receipt_with_context,
    render_receipt_with_context_and_inventory,
};
pub use refresh::{render_refresh_human, render_refresh_human_styled, render_refresh_json};
pub use report_json::{render_json, render_json_with_context, render_json_with_context_and_diff};
pub use report_text::{
    render_human, render_human_with_context, render_markdown, render_markdown_with_context,
};
pub use sarif::{render_sarif, render_sarif_with_context};
pub use spec_system_render::{
    SpecSystemRenderFormat, filter_spec_system_report_for_artifact, json_escape,
    optional_bool_json, render_spec_system_explain_markdown, render_spec_system_json,
    render_spec_system_markdown, render_spec_system_report, spec_system_mode_name,
    spec_system_proof_commands,
};
pub use spec_system_report_types::{
    SpecSystemArtifact, SpecSystemFederationSummary, SpecSystemFinding, SpecSystemImportDiagnostic,
    SpecSystemImportEdge, SpecSystemImportGraphSummary, SpecSystemImportNode,
    SpecSystemLedgerContributor, SpecSystemLink, SpecSystemReadiness, SpecSystemReadinessCheck,
    SpecSystemReport, SpecSystemWorkItem, spec_system_blocking_reason,
    spec_system_work_item_blocking_reason,
};
pub use style::{
    ColorChoice, Style, StyleEnv, StyleReason, resolve as resolve_style, sanitize_terminal_text,
};
pub use summary::{
    Summary, matched_occurrence_counts, matched_policy_missing_evidence_entries,
    occurrence_headroom_entries, occurrence_headroom_for_entry, policy_baseline_debt_entries,
    policy_missing_evidence_entries,
};
pub use why::{render_why_json, render_why_json_with_result_class, render_why_target_scan_json};
pub use worklist::{render_worklist_human, render_worklist_human_styled, render_worklist_json};

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
mod spec_system_render;
mod spec_system_report_types;
#[cfg(test)]
mod text_tests;
#[cfg(test)]
mod worklist_tests;

pub use artifacts::{
    FINAL_EVIDENCE_EDGE_SCHEMA_ID, FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
    FINAL_EVIDENCE_EVALUATION_SCHEMA_ID, FINAL_EVIDENCE_EVALUATION_SCHEMA_VERSION,
    FINAL_EVIDENCE_GRAPH_SCHEMA_ID, FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
    FINAL_EVIDENCE_NODE_SCHEMA_ID, FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1,
    FinalEvidenceEdgeV1, FinalEvidenceEvaluationResultV1, FinalEvidenceFindingKindV1,
    FinalEvidenceFindingV1, FinalEvidenceGraphEvaluationV1, FinalEvidenceGraphModeV1,
    FinalEvidenceGraphV1, FinalEvidenceInvalidationDimensionV1, FinalEvidenceNodeClassV1,
    FinalEvidenceNodeDispositionV1, FinalEvidenceNodeResultV1, FinalEvidenceNodeV1,
    FinalEvidenceOriginV1, FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1,
    FinalEvidenceProducerExpectationV1, FinalEvidenceProducerV1, FinalEvidenceReleaseIdentityV1,
    FinalEvidenceSelectedSubjectV1, FinalEvidenceSubjectBindingV1, evaluate_final_evidence_graph,
    final_evidence_graph_digest, render_final_evidence_evaluation_json,
    render_final_evidence_evaluation_markdown, render_final_evidence_graph_canonical_bytes,
    render_final_evidence_graph_canonical_json,
};
