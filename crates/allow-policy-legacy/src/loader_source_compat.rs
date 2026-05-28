use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use std::path::Path;
use toml::Value;

use crate::converter_file_configs::{
    config_from_current_non_rust_findings, config_from_generated_rules,
};
use crate::converter_panic_configs::{
    config_from_no_panic_allowlist_entries, config_from_no_panic_baseline_entries,
};
use crate::converters::{config_from_clippy_rules, config_from_unsafe_rules};
use crate::io::{legacy_table, read_policy};
use crate::parsers::{
    is_clippy_exceptions_policy, parse_clippy_rules, parse_generated_rules,
    parse_no_panic_allowlist_entries, parse_no_panic_baseline_entries, parse_non_rust_rules,
    parse_unsafe_rules,
};

pub fn load_non_rust_compat_config(
    path: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("non-rust-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a non-rust-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_non_rust_rules(&table)?;
    let cfg = config_from_current_non_rust_findings(&table, &rules, findings)?;
    Ok(cfg)
}

pub fn load_generated_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("generated-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a generated-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_generated_rules(&table)?;
    config_from_generated_rules(&table, &rules)
}

pub fn load_no_panic_baseline_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("no-panic-baseline") {
        return Err(CargoAllowError::new(format!(
            "{} is not a no-panic-baseline policy",
            path.as_ref().display()
        )));
    }
    let entries = parse_no_panic_baseline_entries(&table)?;
    config_from_no_panic_baseline_entries(&table, &entries)
}

pub fn load_no_panic_allowlist_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("no-panic-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a no-panic-allowlist policy",
            path.as_ref().display()
        )));
    }
    let entries = parse_no_panic_allowlist_entries(&table)?;
    config_from_no_panic_allowlist_entries(&table, &entries)
}

pub fn load_clippy_exceptions_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if !is_clippy_exceptions_policy(&table) {
        return Err(CargoAllowError::new(format!(
            "{} is not a clippy-exceptions policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_clippy_rules(&table)?;
    config_from_clippy_rules(&table, &rules)
}

pub fn load_unsafe_allowlist_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("unsafe-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not an unsafe-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_unsafe_rules(&table)?;
    config_from_unsafe_rules(&table, &rules)
}
