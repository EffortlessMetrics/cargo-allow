use allow_core::SimpleDate;

mod converter_clippy_entries;
mod converter_config;
mod converter_dependency_entries;
mod converter_executable_entries;
mod converter_file_configs;
mod converter_file_entries;
mod converter_file_support;
mod converter_generated_entries;
mod converter_panic_configs;
mod converter_panic_entries;
mod converter_panic_support;
mod converter_process_network_entries;
mod converter_process_network_support;
mod converter_unsafe_entries;
mod converter_workflow_entries;
mod converter_workflow_support;
mod converters;
mod fields;
mod finding_config;
mod finding_dependency;
mod finding_generated_executable;
mod finding_workflow;
mod findings;
mod io;
mod loader_compat;
mod loader_legacy_dispatch;
mod loader_policy_compat;
mod loader_policy_dir;
mod loader_source_compat;
mod loaders;
mod parser_clippy_entries;
mod parser_dependency_entries;
mod parser_executable_entries;
mod parser_file_entries;
mod parser_panic_entries;
mod parser_process_network_entries;
mod parser_source_entries;
mod parser_support;
mod parser_unsafe_entries;
mod parser_workflow_entries;
mod parsers;
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
    dependency_surface_findings_from_git, executable_findings_from_git,
    generated_findings_from_gitattributes, network_findings_from_config,
    process_findings_from_config, workflow_findings_from_files,
};
pub use loaders::{
    load_clippy_exceptions_compat_config, load_dependency_surface_compat_config,
    load_executable_compat_config, load_generated_compat_config, load_legacy_or_canonical,
    load_legacy_policy_dir, load_legacy_policy_dir_with_non_rust_findings,
    load_network_compat_config, load_no_panic_allowlist_compat_config,
    load_no_panic_baseline_compat_config, load_non_rust_compat_config, load_process_compat_config,
    load_unsafe_allowlist_compat_config, load_workflow_compat_config, migration_notes,
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
mod generated_executable_tests;
#[cfg(test)]
mod lint_unsafe_tests;
#[cfg(test)]
mod no_panic_tests;
#[cfg(test)]
mod non_rust_tests;
#[cfg(test)]
mod policy_dir_tests;
#[cfg(test)]
mod process_network_tests;
#[cfg(test)]
mod test_findings;
#[cfg(test)]
mod test_fixture_text;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod workflow_dependency_tests;
