use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_policy_configs::{
    config_from_dependency_surface_rules, config_from_executable_rules, config_from_network_rules,
    config_from_process_rules, config_from_workflow_rules,
};
use crate::io::{legacy_table, read_policy};
use crate::parsers::{
    parse_dependency_surface_rules, parse_executable_rules, parse_network_rules,
    parse_process_rules, parse_workflow_rules,
};

pub fn load_executable_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("executable-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not an executable-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_executable_rules(&table)?;
    config_from_executable_rules(&table, &rules)
}

pub fn load_workflow_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("workflow-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a workflow-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_workflow_rules(&table)?;
    config_from_workflow_rules(&table, &rules)
}

pub fn load_dependency_surface_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("dependency-surface-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a dependency-surface-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_dependency_surface_rules(&table)?;
    config_from_dependency_surface_rules(&table, &rules)
}

pub fn load_process_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("process-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a process-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_process_rules(&table)?;
    config_from_process_rules(&table, &rules)
}

pub fn load_network_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("network-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a network-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_network_rules(&table)?;
    config_from_network_rules(&table, &rules)
}
