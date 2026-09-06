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
mod resolved_config;
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
    AdoptionPolicyFacts, BaseScanCompletenessV1, BootstrapDisposition,
    CANDIDATE_APPLY_CLAIM_BOUNDARY_V1, CANDIDATE_APPLY_RECEIPT_SCHEMA_V1,
    CANDIDATE_OPERATION_PLAN_SCHEMA_V1, CANDIDATE_PREPARATION_CLAIM_BOUNDARY_V1,
    CANDIDATE_PREPARATION_PLAN_SCHEMA_V1, CANDIDATE_PREPARATION_RECEIPT_CLAIM_BOUNDARY_V1,
    CANDIDATE_PREPARATION_RECEIPT_SCHEMA_V1, CANDIDATE_PREPARATION_RESULT_SCHEMA_V1,
    CandidateApplyLockRecordV1, CandidateApplyOperationRecordV1, CandidateApplyReceiptV1,
    CandidateApplyStateV1, CandidateChangedFileV1, CandidateCollisionResultV1,
    CandidateContentStateV1, CandidateCorpusRoleV1, CandidateCorpusSourceV1,
    CandidateExternalObservationV1, CandidateFileOperationV1, CandidateGovernedFileClassV1,
    CandidateGraphRowV1, CandidateOperationCompilerInput, CandidateOperationPlanV1,
    CandidateOperationPostureV1, CandidatePackageRowV1, CandidatePreparationDecisionV1,
    CandidatePreparationDirtyStateV1, CandidatePreparationInputIdentityV1,
    CandidatePreparationOperationV1, CandidatePreparationPlanV1, CandidatePreparationReadinessV1,
    CandidatePreparationReceiptV1, CandidatePreparationResultV1, CandidatePreparationStateV1,
    CandidateProjectionInput, CandidateReleaseIdentityProjectionV1, CandidateResolvedDecisionV1,
    CandidateSelectedRowV1, CandidateSupportChannelPostureV1, CandidateSurfaceDecisionV1,
    CandidateSurfaceInputV1, CandidateValidationObligationV1, CandidateValidationResultV1,
    CandidateValidationRowV1, CargoAllowGitHubPrAnnotationV1, CargoAllowGitHubPrCheckReceiptV1,
    CargoAllowGitHubPrCheckV1, ChannelPostureV1, ConfigProvenanceSummary, ConfiguredLedgerSummary,
    CoreAdoptionPlanV1, DiffEvidenceChange, DiffExceptionIdentityChange, DiffFindingChange,
    DiffLedgerMovementSummary, DiffLifecycleChange, DiffMetadataChange, DiffMovementCounts,
    DiffOccurrenceLimitChange, DiffPolicyChange, DiffPolicyStatusChange, DiffPostureDeltaCounts,
    DiffPostureSummary, DiffReport, DiffRequirementChange, DiffScopeChange,
    DiffSelectorIdentityChange, DiffSelectorPrecisionChange, DoctorReport,
    EVALUATION_ARTIFACT_SET_V1_SCHEMA_ID, EVALUATION_ARTIFACT_SET_V1_SCHEMA_VERSION,
    EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_ID, EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_VERSION,
    EvaluationArtifactEntryV1, EvaluationArtifactSetResultV2, EvaluationArtifactSetV1,
    EvaluationArtifactSetV1Validation, EvaluationContext, EvaluationResultClass,
    EvaluationResultClassV2, EvidenceReference, ExactCandidateJourneyStepV2,
    ExactCandidatePackageRowV2, ExactCandidatePayloadV2, ExactCandidateResultV2,
    ExactCandidateV2Validation, ExplainReport, FederationDiagnosticSummary,
    FederationDivergenceKindCount, FederationDivergenceRecordSummary, FederationDivergenceSummary,
    FederationReportContext, FileFamilyConflictSummary, FileFamilyRuleSummary,
    FinalCandidateEligibilityV1, GITHUB_PR_CHECK_V1_SCHEMA_ID, GITHUB_PR_CHECK_V1_SCHEMA_VERSION,
    GitHubPrAnnotationClassV1, GitHubPrCheckResultV1, GitHubPrCheckSubjectV1,
    GitHubPrDiffReportViewV1, GitHubPrDiffViewV1, GitHubPrFindingChangeRowViewV1,
    GitHubPrInventoryViewV1, GitHubReleaseObservationV1, ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_ID,
    ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_VERSION, InventoryCompleteness, InventoryMode,
    IsolatedInstallGraphComparisonV2, IsolatedInstallPackageRowV2, IsolatedInstallPayloadV2,
    IsolatedInstallResultV2, IsolatedInstallV2Validation, LedgerContributorSummary, ListColumn,
    ListFilters, ListRow, MigrateReport, ObservationCompletenessV1, PACKAGE_CANDIDATE_V2_SCHEMA_ID,
    PACKAGE_CANDIDATE_V2_SCHEMA_VERSION, PackageCandidateDependencyKindV2,
    PackageCandidateDependencyRowV2, PackageCandidateFamilyV2, PackageCandidatePayloadV2,
    PackageCandidateResultV2, PackageCandidateRowV2, PackageCandidateV2Validation,
    PackageRowClassV1, PolicyState, ProposeReport, PruneCandidate, PruneModeContext,
    PublicationClassificationV1, PublicationStateV1, RC_PUBLICATION_INCIDENT_SCHEMA,
    RECONCILED_PACKAGE_PUBLICATION_SCHEMA, RELEASE_MANIFEST_V2_SCHEMA_ID,
    RELEASE_MANIFEST_V2_SCHEMA_VERSION, REQUIRED_SURFACE_OWNERS, RcPublicationIncidentV1,
    ReconciledPackagePublicationV1, RefreshModeContext, RefreshReport, RegistryObservationV1,
    ReleaseAttemptV1, ReleaseChannelV1, ReleaseIdentityErrorV1, ReleaseIdentityV1,
    ReleaseManifestAuthenticationV2, ReleaseManifestEnvelopeV2, ReleaseManifestOperationV2,
    ReleaseManifestPackageRowV2, ReleaseManifestPayloadV2, ReleaseManifestPublicationPostureV2,
    ReleaseManifestResultV2, ReleaseManifestSupportPostureV2, ReleaseManifestV2Validation,
    ReleaseVersionV1, RendererFormatV1, RowReconciliationV1, SUPPORTED_TOPOLOGY_GENERATION_V1,
    TagObservationV1, WhyCandidateEntry, WhyProofPlan, WhyReport, WhyTargetScan,
    WhyTargetScanReport, WorklistFilters, WorklistItem, WritePosture, compile_candidate_operations,
    manifest_rows_from_reconciled, prepare_candidate_plan, project_github_pr_check,
    recommend_core_adoption_plan, render_evaluation_artifact_set_v1,
    render_evaluation_artifact_set_v1_bytes, render_exact_candidate_v2,
    render_exact_candidate_v2_bytes, render_isolated_install_v2, render_isolated_install_v2_bytes,
    render_package_candidate_v2, render_package_candidate_v2_bytes,
    render_release_manifest_v2_envelope, render_release_manifest_v2_envelope_bytes,
    render_release_manifest_v2_payload, render_release_manifest_v2_payload_bytes,
    validate_candidate_operation_set, validate_exact_candidate_v2, validate_github_pr_check_v1,
    validate_isolated_install_v2, validate_package_candidate_v2, validate_release_manifest_v2,
};

pub use artifacts::{
    LandedEffectStatusV1, LandedEffectV1, NextFrontierV1, PostMergeReconciliationRequestV1,
    PostMergeReconciliationResultV1, ReconciliationDispositionV1,
};

// Supported feature-configuration matrix and proof contracts (#3905 PR A).
pub use artifacts::{
    CrateFeatureInventoryV1, CrateOptionalDependencyV1, FEATURE_CONFIGURATION_MATRIX_ID,
    FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_ID,
    FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_VERSION,
    FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_ID,
    FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_VERSION, FeatureConfigurationCommandResultV1,
    FeatureConfigurationCommandStatusV1, FeatureConfigurationEnforcementPostureV1,
    FeatureConfigurationGapV1, FeatureConfigurationMatrixResultV1,
    FeatureConfigurationMatrixValidationV1, FeatureConfigurationNonSelectionV1,
    FeatureConfigurationProductV1, FeatureConfigurationProofDepthV1,
    FeatureConfigurationProofOutcomeV1, FeatureConfigurationProofReceiptV1,
    FeatureConfigurationProofRequestV1, FeatureConfigurationReceiptResultV1,
    FeatureConfigurationReceiptValidationV1, FeatureConfigurationRequestResultV1,
    FeatureConfigurationRequestValidationV1, FeatureConfigurationSupportTierV1,
    FeatureConfigurationTargetClassV1, FeatureImplicationV1, NoDefaultFeaturesPostureV1,
    SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_ID,
    SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_VERSION, SupportedFeatureConfigurationV1,
    WORKSPACE_RUST_VERSION, crate_feature_inventory, current_feature_configuration_gaps,
    effective_feature_set, render_feature_configuration_proof_receipt_v1,
    render_feature_configuration_proof_receipt_v1_bytes,
    render_feature_configuration_proof_request_v1,
    render_supported_feature_configuration_matrix_v1,
    render_supported_feature_configuration_matrix_v1_bytes,
    row_for_supported_feature_configuration, supported_feature_configuration_matrix,
    supported_feature_configuration_rows, validate_feature_configuration_proof_receipt_v1,
    validate_feature_configuration_proof_request_v1,
    validate_supported_feature_configuration_matrix_v1,
};

// Root re-exports for the four release-artifact modules that #4007 dropped
// from `artifacts.rs` while switching `final_evidence_graph_v1` to a glob
// export. The modules and their contents were never removed, only unwired,
// so their consumers in `crates/cargo-allow/tests/` stopped resolving.
pub use artifacts::{
    ActualDownloadedFileV1, AggregateOperationStateV1, ArtifactTransferDispositionV1,
    ArtifactTransferFileV1, ArtifactTransferInitV1, CandidateCustodyInitV1,
    CargoAllowFrozenCandidateCustodyV1, CargoAllowPostMergeQualificationV1,
    CargoAllowReleaseArtifactTransferV1, CargoAllowReleaseOperationV1, ConfidentialityClassV1,
    ConsumerContextV1, CustodyDispositionV1, CustodyFileV1, MergeMethodV1, MergedStateV1,
    OperationClassV1, OperationEventKindV1, OperationEventV1, PostMergeEquivalenceVerdictV1,
    PostMergeQualificationInitV1, ProducerIdentityV1, RetainedCustodyItemV1, ReviewedContextV1,
    TrustClassV1, UntrustedInputPostureV1,
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
pub use resolved_config::render_resolved_cargo_allow_config_json;
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

// Root re-exports for the #3929 final pre-freeze readiness aggregate.
pub use artifacts::{
    CargoAllowFinalReadinessV1, FINAL_READINESS_SCHEMA_ID, FINAL_READINESS_SCHEMA_VERSION,
    FinalReadinessClaimNarrowingV1, FinalReadinessCustodyPostureV1, FinalReadinessDecisionInputsV1,
    FinalReadinessDecisionStateV1, FinalReadinessPostMergePostureV1,
    FinalReadinessQualificationPostureV1, FinalReadinessRequiredEvidenceV1,
    FinalReadinessRootDecisionV1, FinalReadinessRowKindV1, FinalReadinessRowV1,
    FinalReadinessSupportedLimitationV1, FinalReadinessVerdictV1, aggregate_final_readiness,
    render_final_readiness_json, render_final_readiness_markdown,
};

// Root re-exports for the #3919 final-freeze replay contract.
pub use artifacts::{
    CargoAllowFinalFreezeReceiptV1, CargoAllowFinalFreezeReplayInputsV1,
    CargoAllowFinalFreezeReplayV1, FINAL_FREEZE_EXPECTED_SHARED_ROWS_V1,
    FINAL_FREEZE_EXPECTED_UPLOAD_ROWS_V1, FINAL_FREEZE_RECEIPT_SCHEMA_ID,
    FINAL_FREEZE_RECEIPT_SCHEMA_VERSION, FINAL_FREEZE_REPLAY_SCHEMA_ID,
    FINAL_FREEZE_REPLAY_SCHEMA_VERSION, FinalFreezeManifestBindingV1, FinalFreezeManifestResultV1,
    FinalFreezeReceiptInitV1, FinalFreezeReplayResultV1, FinalFreezeReplayRowKindV1,
    FinalFreezeReplayRowV1, ObservationFreshnessV1, ObservationReadingRowV1, ObservationReadingV1,
    RefreshableObservationAdapterV1, RefreshableObservationKindV1, RefreshableObservationV1,
    RetainedArtifactBytesV1, RetainedExactArtifactV1, render_final_freeze_replay_json,
    render_final_freeze_replay_markdown, replay_final_freeze,
};

// Root re-exports for the #3845 campaign closeout verifier.
pub use artifacts::{
    CAMPAIGN_ISSUE_CLOSEOUT_SCHEMA_ID, CAMPAIGN_ISSUE_CLOSEOUT_SCHEMA_VERSION,
    CampaignAcceptanceRowV1, CampaignCheckEvidenceV1, CampaignCheckOutcomeV1,
    CampaignCloseoutRecordV1, CampaignCloseoutResultV1, CampaignCloseoutVerdictV1,
    CampaignEvidenceClassV1, CampaignPrEvidenceV1, CampaignPrStateV1, CampaignRepositoryStateV1,
    CampaignReviewPairV1, evaluate_campaign_closeout,
};

// Root re-exports for the #3928 frozen-subject lock contract.
pub use artifacts::{
    CargoAllowFrozenSubjectLockV1, FROZEN_SUBJECT_LOCK_SCHEMA_ID,
    FROZEN_SUBJECT_LOCK_SCHEMA_VERSION, FrozenSubjectChangeV1, FrozenSubjectInvalidationV1,
    FrozenSubjectLockInputV1, FrozenSubjectPathClassV1, FrozenSubjectPathKindV1,
    FrozenSubjectReceiptIdentityV1, FrozenSubjectStateV1, FrozenSubjectVerdictV1,
    classify_frozen_subject_path, evaluate_frozen_subject_lock,
};

// Root re-exports for the #3737 final support-selection freeze contract.
pub use artifacts::{
    FINAL_SELECTION_IDENTITY_ROLE, FINAL_SUPPORT_SELECTION_SCHEMA_ID,
    FINAL_SUPPORT_SELECTION_SCHEMA_VERSION, FinalSelectionDispositionV1, FinalSelectionRowV1,
    FinalSupportSelectionErrorV1, FinalSupportSelectionV1,
};
