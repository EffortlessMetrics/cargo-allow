use std::process;

mod add;
mod audit;
mod check;
mod cli;
mod cli_types;
mod command_support;
mod companion;
mod compat;
mod diff;
#[cfg(test)]
mod diff_json_test_support;
mod doctor;
mod evidence_inventory;
mod evidence_render;
mod explain;
mod federation_doctor;
mod federation_report;
mod init;
mod io;
mod kind_filter;
mod list;
mod migrate;
mod mutation_lock;
mod policy_config;
mod propose;
mod prune;
mod refresh;
mod reporting;
mod selector;
mod spec_system;
mod worklist;
mod world;

pub(crate) use command_support::*;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("error: {err}");
        // Exit 1 for runtime/validation failures. Clap usage errors are
        // handled by clap internally (exit 2) and never reach here.
        // Policy-violation exits (check/diff gate failures) call
        // process::exit(1) directly in their command handlers.
        process::exit(1);
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
mod artifact_schema_worklist_tests;
#[cfg(test)]
mod artifact_top_level_contract_tests;
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
mod spec_system_profile_tests;
