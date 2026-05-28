use allow_core::{AllowConfig, CargoAllowResult, Finding};

use crate::converter_config::config_from_entries;
use crate::converter_file_entries::{entry_from_finding, entry_from_rule};
use crate::converter_file_support::best_rule_index;
use crate::converter_generated_entries::entry_from_generated_rule;
use crate::types::{LegacyGeneratedRule, LegacyNonRustRule};

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
