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
mod reporting;
mod selector;
mod source_tree;
mod worklist;
mod world;

pub(crate) use command_support::*;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("error: {err}");
        process::exit(2);
    }
}

#[cfg(test)]
mod artifact_contract_tests;
#[cfg(test)]
mod artifact_schema_tests;
#[cfg(test)]
mod compat_companion_tests;
#[cfg(test)]
mod compat_dependency_tests;
#[cfg(test)]
mod compat_integration_tests;
#[cfg(test)]
mod compat_test_support;
#[cfg(test)]
mod report_config_tests;
#[cfg(test)]
mod root_cli_tests;
