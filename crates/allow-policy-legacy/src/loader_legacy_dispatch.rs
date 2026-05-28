use allow_core::{AllowConfig, CargoAllowResult};
use toml::Value;

use crate::converters::{
    config_from_clippy_rules, config_from_dependency_surface_rules, config_from_executable_rules,
    config_from_generated_rules, config_from_network_rules, config_from_no_panic_allowlist_entries,
    config_from_no_panic_baseline_entries, config_from_non_rust_rules, config_from_process_rules,
    config_from_unsafe_rules, config_from_workflow_rules,
};
use crate::parsers::{
    is_clippy_exceptions_policy, parse_clippy_rules, parse_dependency_surface_rules,
    parse_executable_rules, parse_generated_rules, parse_network_rules,
    parse_no_panic_allowlist_entries, parse_no_panic_baseline_entries, parse_non_rust_rules,
    parse_process_rules, parse_unsafe_rules, parse_workflow_rules,
};

pub(crate) fn config_from_legacy_table(
    table: &toml::Table,
) -> CargoAllowResult<Option<AllowConfig>> {
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
