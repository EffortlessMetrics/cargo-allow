use allow_core::{
    AllowConfig, AllowEntry, CargoAllowResult, Finding, Requirements, WorkspaceConfig,
};
use allow_policy::validate_policy;

use crate::converter_clippy_entries::entry_from_clippy_rule;
use crate::converter_dependency_entries::entry_from_dependency_surface_rule;
use crate::converter_executable_entries::entry_from_executable_rule;
use crate::converter_file_entries::{
    entry_from_finding, entry_from_generated_rule, entry_from_rule,
};
use crate::converter_file_support::best_rule_index;
use crate::converter_panic_entries::{
    entry_from_no_panic_allow_entry, entry_from_no_panic_baseline_entry,
};
use crate::converter_process_network_entries::{entry_from_network_rule, entry_from_process_rule};
use crate::converter_unsafe_entries::entry_from_unsafe_rule;
use crate::converter_workflow_entries::entries_from_workflow_rule;
use crate::fields::string_field;
use crate::types::{
    LegacyClippyRule, LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyGeneratedRule,
    LegacyNetworkRule, LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry, LegacyNonRustRule,
    LegacyProcessRule, LegacyUnsafeRule, LegacyWorkflowRule,
};

pub(crate) fn config_from_non_rust_rules(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_rule))
}

pub(crate) fn config_from_generated_rules(
    table: &toml::Table,
    rules: &[LegacyGeneratedRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_generated_rule))
}

pub(crate) fn config_from_no_panic_baseline_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicBaselineEntry],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(
        table,
        entries.iter().map(entry_from_no_panic_baseline_entry),
    )
}

pub(crate) fn config_from_no_panic_allowlist_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicAllowEntry],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, entries.iter().map(entry_from_no_panic_allow_entry))
}

pub(crate) fn config_from_clippy_rules(
    table: &toml::Table,
    rules: &[LegacyClippyRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_clippy_rule))
}

pub(crate) fn config_from_unsafe_rules(
    table: &toml::Table,
    rules: &[LegacyUnsafeRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_unsafe_rule))
}

pub(crate) fn config_from_executable_rules(
    table: &toml::Table,
    rules: &[LegacyExecutableRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_executable_rule))
}

pub(crate) fn config_from_workflow_rules(
    table: &toml::Table,
    rules: &[LegacyWorkflowRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().flat_map(entries_from_workflow_rule))
}

pub(crate) fn config_from_dependency_surface_rules(
    table: &toml::Table,
    rules: &[LegacyDependencySurfaceRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_dependency_surface_rule))
}

pub(crate) fn config_from_process_rules(
    table: &toml::Table,
    rules: &[LegacyProcessRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_process_rule))
}

pub(crate) fn config_from_network_rules(
    table: &toml::Table,
    rules: &[LegacyNetworkRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_network_rule))
}

pub(crate) fn config_from_current_non_rust_findings(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(
        table,
        findings.iter().enumerate().filter_map(|(index, finding)| {
            best_rule_index(rules, finding)
                .and_then(|rule_index| rules.get(rule_index))
                .map(|rule| entry_from_finding(rule, finding, index + 1))
        }),
    )
}

fn config_from_entries(
    table: &toml::Table,
    entries: impl IntoIterator<Item = AllowEntry>,
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = entries.into_iter().collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn base_config(table: &toml::Table) -> AllowConfig {
    AllowConfig {
        schema_version: "0.1".to_string(),
        policy: "cargo-allow".to_string(),
        owner: string_field(table, "owner"),
        status: string_field(table, "status"),
        workspace: WorkspaceConfig::default(),
        requirements: Requirements::default(),
        allow: Vec::new(),
    }
}
