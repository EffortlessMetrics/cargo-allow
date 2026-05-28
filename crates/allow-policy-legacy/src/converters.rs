use allow_core::{AllowConfig, CargoAllowResult, Finding, Requirements, WorkspaceConfig};
use allow_policy::validate_policy;

use crate::converter_exception_entries::{entry_from_clippy_rule, entry_from_unsafe_rule};
use crate::converter_file_entries::{
    entry_from_finding, entry_from_generated_rule, entry_from_rule,
};
use crate::converter_panic_entries::{
    entry_from_no_panic_allow_entry, entry_from_no_panic_baseline_entry,
};
use crate::converter_policy_entries::{
    entry_from_dependency_surface_rule, entry_from_executable_rule,
};
use crate::converter_process_network_entries::{entry_from_network_rule, entry_from_process_rule};
use crate::converter_support::best_rule_index;
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
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_generated_rules(
    table: &toml::Table,
    rules: &[LegacyGeneratedRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_generated_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_no_panic_baseline_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicBaselineEntry],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = entries
        .iter()
        .map(entry_from_no_panic_baseline_entry)
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_no_panic_allowlist_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicAllowEntry],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = entries
        .iter()
        .map(entry_from_no_panic_allow_entry)
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_clippy_rules(
    table: &toml::Table,
    rules: &[LegacyClippyRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_clippy_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_unsafe_rules(
    table: &toml::Table,
    rules: &[LegacyUnsafeRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_unsafe_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_executable_rules(
    table: &toml::Table,
    rules: &[LegacyExecutableRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_executable_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_workflow_rules(
    table: &toml::Table,
    rules: &[LegacyWorkflowRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().flat_map(entries_from_workflow_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_dependency_surface_rules(
    table: &toml::Table,
    rules: &[LegacyDependencySurfaceRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules
        .iter()
        .map(entry_from_dependency_surface_rule)
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_process_rules(
    table: &toml::Table,
    rules: &[LegacyProcessRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_process_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_network_rules(
    table: &toml::Table,
    rules: &[LegacyNetworkRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_network_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_current_non_rust_findings(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = findings
        .iter()
        .enumerate()
        .filter_map(|(index, finding)| {
            best_rule_index(rules, finding)
                .and_then(|rule_index| rules.get(rule_index))
                .map(|rule| entry_from_finding(rule, finding, index + 1))
        })
        .collect();
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
