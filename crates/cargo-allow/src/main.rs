use std::process;

mod add;
mod adoption;
mod audit;
mod capabilities;
mod check;
mod cli;
mod cli_types;
mod command_support;
mod companion;
mod compat;
mod completions;
mod core_command_router;
pub mod core_command_summary;
mod diff;
#[cfg(test)]
mod diff_json_test_support;
mod doctor;
mod error_report;
mod evidence_inventory;
mod evidence_render;
mod exit_code;
mod explain;
mod federation_doctor;
mod federation_report;
mod hooks;
mod init;
mod intent_delegate;
mod intent_provider;
mod kind_filter;
mod list;
mod migrate;
mod mutation_lock;
mod plan_bindings;
mod policy_config;
pub mod precommit_tool;
mod propose;
mod prune;
mod reference;
mod refresh;
mod reporting;
mod selector;
mod spec_precommit;
mod spec_system;
pub mod spec_system_graph_movement;
pub mod spec_system_parity_corpus;
mod spec_system_source;
mod spec_system_view;
pub mod spec_system_workspace;
pub mod spec_system_workspace_composition;
mod support_bundle;
mod vocabulary;
mod why;
mod worklist;
mod world;

pub(crate) use command_support::*;

fn main() {
    if let Err(err) = cli::run() {
        error_report::report_cli_error(&err);
        // Clap usage errors exit 2 before reaching here. Structured
        // `CargoAllowErrorKind::Usage` errors use the same exit code via
        // `exit_code::exit_code_for_error`. Policy-gate failures in check/diff
        // still call `process::exit(1)` directly in their handlers.
        process::exit(exit_code::exit_code_for_error(&err));
    }
}

#[cfg(test)]
mod artifact_command_contract_tests;
#[cfg(test)]
mod artifact_contract_samples;
#[cfg(test)]
mod artifact_contract_support;
#[cfg(test)]
mod artifact_contract_support_tests;
#[cfg(test)]
mod artifact_contract_tests;
#[cfg(test)]
mod artifact_sample_schema_patterns;
#[cfg(test)]
mod artifact_sample_schema_support;
#[cfg(test)]
mod artifact_schema_add_tests;
#[cfg(test)]
mod artifact_schema_diff_tests;
#[cfg(test)]
mod artifact_schema_doctor_tests;
#[cfg(test)]
mod artifact_schema_evidence_reference_tests;
#[cfg(test)]
mod artifact_schema_expectations;
#[cfg(test)]
mod artifact_schema_explain_tests;
#[cfg(test)]
mod artifact_schema_identity_tests;
#[cfg(test)]
mod artifact_schema_index_tests;
#[cfg(test)]
mod artifact_schema_list_tests;
#[cfg(test)]
mod artifact_schema_migrate_tests;
#[cfg(test)]
mod artifact_schema_policy_metadata_tests;
#[cfg(test)]
mod artifact_schema_propose_tests;
#[cfg(test)]
mod artifact_schema_prune_tests;
#[cfg(test)]
mod artifact_schema_receipt_tests;
#[cfg(test)]
mod artifact_schema_refresh_tests;
#[cfg(test)]
mod artifact_schema_report_diff_identity_tests;
#[cfg(test)]
mod artifact_schema_report_diff_policy_detail_tests;
#[cfg(test)]
mod artifact_schema_report_diff_tests;
#[cfg(test)]
mod artifact_schema_report_tests;
#[cfg(test)]
mod artifact_schema_selector_lifecycle_tests;
#[cfg(test)]
mod artifact_schema_shared_fragment_tests;
#[cfg(test)]
mod artifact_schema_shared_tests;
#[cfg(test)]
mod artifact_schema_source_location_tests;
#[cfg(test)]
mod artifact_schema_spec_system_tests;
#[cfg(test)]
mod artifact_schema_strictness_tests;
#[cfg(test)]
mod artifact_schema_summary_tests;
#[cfg(test)]
mod artifact_schema_support;
#[cfg(test)]
mod artifact_schema_why_tests;
#[cfg(test)]
mod artifact_schema_worklist_tests;
#[cfg(test)]
mod artifact_top_level_contract_tests;
#[cfg(test)]
mod cargo_proof_parity_tests;
#[cfg(test)]
mod compat_companion_tests;
#[cfg(test)]
mod compat_dependency_tests;
#[cfg(test)]
mod compat_integration_tests;
#[cfg(test)]
mod compat_panic_integration_tests;
#[cfg(test)]
mod compat_test_support;
#[cfg(test)]
mod effortless_repo_edit_parity_tests;
#[cfg(test)]
mod effortless_repo_snapshot_parity_tests;
#[cfg(test)]
mod effortless_rust_source_index_parity_tests;
#[cfg(test)]
mod extraction_shim_registry_tests;
#[cfg(test)]
mod intent_edit_parity_tests;
#[cfg(test)]
mod intent_engine_parity_tests;
#[cfg(test)]
mod intent_model_parity_tests;
#[cfg(test)]
mod intent_protocol_parity_tests;
#[cfg(test)]
mod product_crate_architecture_tests;
#[cfg(test)]
mod product_move_ledger_tests;
#[cfg(test)]
mod product_package_topology_tests;
#[cfg(test)]
mod proof_adapter_cargo_allow_parity_tests;
#[cfg(test)]
mod proof_adapter_command_parity_tests;
#[cfg(test)]
mod proof_adapter_hawk_parity_tests;
#[cfg(test)]
mod proof_adapter_ripr_parity_tests;
#[cfg(test)]
mod proof_engine_parity_tests;
#[cfg(test)]
mod proof_protocol_parity_tests;
#[cfg(test)]
mod proof_provider_api_parity_tests;
#[cfg(test)]
mod readme_tests;
#[cfg(test)]
mod release_prep_tests;
#[cfg(test)]
mod report_config_tests;
#[cfg(test)]
mod root_cli_compat_tests;
#[cfg(test)]
mod root_cli_tests;
#[cfg(test)]
mod spec_design_artifact_links_tests;
#[cfg(test)]
mod spec_system_profile_tests;

#[cfg(test)]
#[path = "no_new_marker_guard_tests.rs"]
mod no_new_marker_guard_tests;
