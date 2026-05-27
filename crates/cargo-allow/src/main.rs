use std::process;

mod add;
mod audit;
mod check;
mod cli;
mod cli_types;
mod companion;
mod compat;
mod diff;
mod doctor;
mod explain;
mod init;
mod io;
mod kind_filter;
mod list;
mod migrate;
mod policy_config;
mod propose;
mod prune;
mod render;
mod reporting;
mod worklist;
mod world;

#[cfg(test)]
pub(crate) use cli::{CargoAllowCli, CargoAllowCommand, normalized_args};
pub(crate) use cli_types::{InventoryFacts, OutputFormat, RootArgs};
pub(crate) use companion::{canonical_companion_findings, extend_unique_findings};
pub(crate) use compat::load_compat_world;
pub(crate) use io::{write_file, write_file_no_overwrite};
pub(crate) use kind_filter::{
    FamilyFilter, KindFilter, is_clippy_compat_kind, is_dependency_surface_compat_kind,
    is_executable_compat_kind, is_network_compat_kind, is_no_panic_allowlist_compat_kind,
    is_panic_compat_kind, is_process_compat_kind, is_unsafe_compat_kind, is_workflow_compat_kind,
    parse_kind_filter,
};
pub(crate) use policy_config::{
    config_path, git_relative_config_path, load_config_optional, load_config_required,
    root_relative_path,
};
pub(crate) use render::{
    markdown_cell, scope_has_wildcard, selector_from_finding, source_package_name,
    source_tree_path_matches_filter, source_tree_root_text,
};
pub(crate) use reporting::{
    ReportRenderArgs, policy_baseline_debt_entries, print_report, report_config,
};
pub(crate) use world::{load_world, load_world_with_evidence_validation};

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("error: {err}");
        process::exit(2);
    }
}

#[cfg(test)]
mod artifact_contract_tests;
#[cfg(test)]
mod compat_integration_tests;
#[cfg(test)]
mod main_tests;
#[cfg(test)]
mod report_config_tests;
