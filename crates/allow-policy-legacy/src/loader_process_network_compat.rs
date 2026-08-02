use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_policy_configs::{config_from_network_rules, config_from_process_rules};
use crate::io::{legacy_table_at, read_policy};
use crate::parsers::{parse_network_rules, parse_process_rules};
use crate::source_context::at_legacy_source;

pub fn load_process_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy(path)?;
    let result = (|| -> CargoAllowResult<AllowConfig> {
        let table = legacy_table_at(Some(path), &text)?.ok_or_else(|| {
            CargoAllowError::new(format!("{} is not a TOML table", path.display()))
        })?;
        if table.get("policy").and_then(Value::as_str) != Some("process-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a process-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_process_rules(&table)?;
        config_from_process_rules(&table, &rules)
    })();
    result.map_err(|err| at_legacy_source(err, path, &text))
}

pub fn load_network_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy(path)?;
    let result = (|| -> CargoAllowResult<AllowConfig> {
        let table = legacy_table_at(Some(path), &text)?.ok_or_else(|| {
            CargoAllowError::new(format!("{} is not a TOML table", path.display()))
        })?;
        if table.get("policy").and_then(Value::as_str) != Some("network-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a network-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_network_rules(&table)?;
        config_from_network_rules(&table, &rules)
    })();
    result.map_err(|err| at_legacy_source(err, path, &text))
}
