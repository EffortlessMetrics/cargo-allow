//! Legacy policy adapters for cargo-allow migrations.
//!
//! This crate converts supported bespoke allowlist shapes into canonical
//! cargo-allow policy entries while preserving legacy identifiers, owners,
//! reasons, lifecycle hints, evidence strings, and count limits where available.
//! Migration output remains policy data; it does not execute legacy xtasks.

use allow_core::SimpleDate;

mod advisory_drift_fields;
mod converter_clippy_entries;
mod converter_config;
mod converter_dependency_entries;
mod converter_executable_entries;
mod converter_file_configs;
mod converter_file_entries;
mod converter_file_support;
mod converter_generated_entries;
mod converter_lifecycle_support;
mod converter_metadata_support;
mod converter_network_entries;
mod converter_no_panic_allow_entries;
mod converter_no_panic_baseline_entries;
mod converter_non_rust_finding_entries;
mod converter_non_rust_rule_entries;
mod converter_panic_configs;
mod converter_panic_entries;
mod converter_panic_support;
mod converter_policy_configs;
mod converter_process_entries;
mod converter_process_network_entries;
mod converter_process_network_support;
mod converter_source_configs;
mod converter_unsafe_entries;
mod converter_workflow_action_entries;
mod converter_workflow_entries;
mod converter_workflow_file_entries;
mod converter_workflow_support;
mod fields;
mod finding_config;
mod finding_dependency;
mod finding_generated_executable;
mod finding_workflow;
mod findings;
mod io;
mod legacy_import_batch;
mod legacy_sources;
mod loader_compat;
mod loader_executable_compat;
mod loader_file_compat;
mod loader_legacy_dispatch;
mod loader_panic_compat;
mod loader_policy_compat;
mod loader_policy_dir;
mod loader_process_network_compat;
mod loader_source_compat;
mod loader_source_exception_compat;
mod loader_workflow_dependency_compat;
mod loaders;
mod migration_closeout;
mod migration_lane_descriptors;
mod parser_clippy_entries;
mod parser_dependency_entries;
mod parser_executable_entries;
mod parser_generated_entries;
mod parser_network_entries;
mod parser_no_panic_allowlist_entries;
mod parser_no_panic_baseline_entries;
mod parser_non_rust_entries;
mod parser_panic_entries;
mod parser_process_entries;
mod parser_process_network_entries;
mod parser_source_entries;
mod parser_support;
mod parser_unsafe_entries;
mod parser_workflow_entries;
mod parsers;
mod semantic_selector_fields;
mod source_context;
mod types;
mod types_dependency_entries;
mod types_executable_entries;
mod types_lint_entries;
mod types_panic_entries;
mod types_process_network_entries;
mod types_source_entries;
mod types_unsafe_entries;
mod types_workflow_entries;

pub use findings::{
    dependency_surface_findings_from_git, dependency_surface_findings_from_paths,
    executable_findings_from_git, executable_findings_from_paths,
    generated_findings_from_gitattributes, generated_findings_from_gitattributes_text,
    network_findings_from_config, process_findings_from_config, workflow_findings_from_files,
    workflow_findings_from_sources,
};

/// Policy-derived and compatibility findings emitted by the legacy adapters.
/// These projections do not execute the referenced behavior.
pub const POLICY_FINDING_FAMILIES: &[(&str, &str)] = &[
    ("policy_exception", "github_workflow"),
    ("policy_exception", "workflow_external_action"),
    ("policy_exception", "dependency_surface"),
    ("policy_exception", "process_spawn"),
    ("policy_exception", "network_destination"),
    ("policy_exception", "executable_file"),
];
pub use legacy_import_batch::{LegacyImportBatch, LegacyImportFamily, import_legacy_policy_dir};
pub use legacy_sources::{
    LegacyPolicySource, legacy_compat_kind, legacy_policy_source_for_path,
    list_legacy_policy_sources_in_dir,
};
pub use loaders::{
    load_clippy_exceptions_compat_config, load_dependency_surface_compat_config,
    load_executable_compat_config, load_generated_compat_config, load_legacy_or_canonical,
    load_legacy_policy_dir, load_legacy_policy_dir_with_non_rust_findings,
    load_network_compat_config, load_no_panic_allowlist_compat_config,
    load_no_panic_baseline_compat_config, load_non_rust_compat_config, load_process_compat_config,
    load_unsafe_allowlist_compat_config, load_workflow_compat_config, migration_notes,
};
pub use migration_closeout::{
    BASELINE_DEBT_ITEM_KIND, MISSING_EVIDENCE_ITEM_KIND, MigrationCloseoutBaselineDebt,
    MigrationDebtClass, NO_NEW_GATE_ITEM_KIND, NO_NEW_GATE_SIGNAL, baseline_debt_closeout_metadata,
    migration_closeout_baseline_debt, migration_debt_classes, primary_legacy_descriptor,
};
pub use migration_lane_descriptors::{
    CloseoutQueueHints, CompatKind, DebtPolicy, EvidencePolicy, ExpectedCanonicalShape,
    LegacyInputKind, LegacyLaneDescriptor, LifecyclePolicy, MigrationLane,
    all_legacy_lane_descriptors, descriptor_for_compat_kind_id, descriptor_for_legacy_filename,
    descriptor_for_legacy_policy_key, legacy_lane_descriptor, legacy_policy_filenames,
};

const BASELINE_DEBT_DEFAULT_DAYS: i64 = 67;

fn default_baseline_created() -> String {
    SimpleDate::today_utc_approx().to_string()
}

fn default_baseline_expires() -> String {
    SimpleDate::today_utc_approx()
        .add_days(BASELINE_DEBT_DEFAULT_DAYS)
        .to_string()
}

#[cfg(test)]
mod advisory_drift_import_tests;
#[cfg(test)]
mod evidence_matrix_tests;
#[cfg(test)]
mod generated_executable_tests;
#[cfg(test)]
mod import_parity_metadata_acceptance_tests;
#[cfg(test)]
mod lint_unsafe_tests;
#[cfg(test)]
mod metadata_matrix_tests;
#[cfg(test)]
mod migration_fixture_matrix_tests;
#[cfg(test)]
mod no_panic_tests;
#[cfg(test)]
mod non_rust_tests;
#[cfg(test)]
mod policy_dir_tests;
#[cfg(test)]
mod process_network_tests;
#[cfg(test)]
mod semantic_selector_import_tests;
#[cfg(test)]
mod test_findings;
#[cfg(test)]
mod test_fixture_text;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod workflow_dependency_tests;
