use allow_core::{AllowConfig, CargoAllowResult};

use crate::converter_clippy_entries::entry_from_clippy_rule;
use crate::converter_config::config_from_entries;
use crate::converter_dependency_entries::entry_from_dependency_surface_rule;
use crate::converter_executable_entries::entry_from_executable_rule;
use crate::converter_panic_entries::{
    entry_from_no_panic_allow_entry, entry_from_no_panic_baseline_entry,
};
use crate::converter_process_network_entries::{entry_from_network_rule, entry_from_process_rule};
use crate::converter_unsafe_entries::entry_from_unsafe_rule;
use crate::converter_workflow_entries::entries_from_workflow_rule;
use crate::types::{
    LegacyClippyRule, LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyNetworkRule,
    LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry, LegacyProcessRule, LegacyUnsafeRule,
    LegacyWorkflowRule,
};

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
