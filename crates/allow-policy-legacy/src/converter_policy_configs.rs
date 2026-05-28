use allow_core::{AllowConfig, CargoAllowResult};

use crate::converter_config::config_from_entries;
use crate::converter_dependency_entries::entry_from_dependency_surface_rule;
use crate::converter_executable_entries::entry_from_executable_rule;
use crate::converter_process_network_entries::{entry_from_network_rule, entry_from_process_rule};
use crate::converter_workflow_entries::entries_from_workflow_rule;
use crate::types::{
    LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyNetworkRule, LegacyProcessRule,
    LegacyWorkflowRule,
};

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
