use allow_core::{AllowConfig, CargoAllowResult};
use allow_policy::parse_policy;
use std::path::Path;
use toml::Value;

use crate::converters::{
    config_from_clippy_rules, config_from_dependency_surface_rules, config_from_executable_rules,
    config_from_generated_rules, config_from_network_rules, config_from_no_panic_allowlist_entries,
    config_from_no_panic_baseline_entries, config_from_non_rust_rules, config_from_process_rules,
    config_from_unsafe_rules, config_from_workflow_rules,
};
use crate::io::{legacy_table, read_policy};
pub use crate::loader_compat::{
    load_clippy_exceptions_compat_config, load_dependency_surface_compat_config,
    load_executable_compat_config, load_generated_compat_config, load_network_compat_config,
    load_no_panic_allowlist_compat_config, load_no_panic_baseline_compat_config,
    load_non_rust_compat_config, load_process_compat_config, load_unsafe_allowlist_compat_config,
    load_workflow_compat_config,
};
pub use crate::loader_policy_dir::{
    load_legacy_policy_dir, load_legacy_policy_dir_with_non_rust_findings, migration_notes,
};
use crate::parsers::{
    is_clippy_exceptions_policy, parse_clippy_rules, parse_dependency_surface_rules,
    parse_executable_rules, parse_generated_rules, parse_network_rules,
    parse_no_panic_allowlist_entries, parse_no_panic_baseline_entries, parse_non_rust_rules,
    parse_process_rules, parse_unsafe_rules, parse_workflow_rules,
};

pub fn load_legacy_or_canonical(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    if let Some(table) = legacy_table(&text)?
        && let Some(config) = config_from_legacy_table(&table)?
    {
        return Ok(config);
    }
    parse_policy(&text)
}

fn config_from_legacy_table(table: &toml::Table) -> CargoAllowResult<Option<AllowConfig>> {
    match table.get("policy").and_then(Value::as_str) {
        Some("non-rust-allowlist") => {
            let rules = parse_non_rust_rules(table)?;
            config_from_non_rust_rules(table, &rules).map(Some)
        }
        Some("generated-allowlist") => {
            let rules = parse_generated_rules(table)?;
            config_from_generated_rules(table, &rules).map(Some)
        }
        Some("no-panic-allowlist") => {
            let entries = parse_no_panic_allowlist_entries(table)?;
            config_from_no_panic_allowlist_entries(table, &entries).map(Some)
        }
        Some("no-panic-baseline") => {
            let entries = parse_no_panic_baseline_entries(table)?;
            config_from_no_panic_baseline_entries(table, &entries).map(Some)
        }
        _ if is_clippy_exceptions_policy(table) => {
            let rules = parse_clippy_rules(table)?;
            config_from_clippy_rules(table, &rules).map(Some)
        }
        Some("unsafe-allowlist") => {
            let rules = parse_unsafe_rules(table)?;
            config_from_unsafe_rules(table, &rules).map(Some)
        }
        Some("executable-allowlist") => {
            let rules = parse_executable_rules(table)?;
            config_from_executable_rules(table, &rules).map(Some)
        }
        Some("workflow-allowlist") => {
            let rules = parse_workflow_rules(table)?;
            config_from_workflow_rules(table, &rules).map(Some)
        }
        Some("dependency-surface-allowlist") => {
            let rules = parse_dependency_surface_rules(table)?;
            config_from_dependency_surface_rules(table, &rules).map(Some)
        }
        Some("process-allowlist") => {
            let rules = parse_process_rules(table)?;
            config_from_process_rules(table, &rules).map(Some)
        }
        Some("network-allowlist") => {
            let rules = parse_network_rules(table)?;
            config_from_network_rules(table, &rules).map(Some)
        }
        _ => Ok(None),
    }
}
