use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_policy_configs::{config_from_network_rules, config_from_process_rules};
use crate::io::legacy_table_at;
use crate::parsers::{parse_network_rules, parse_process_rules};
use crate::source_context::with_legacy_source;

pub fn load_process_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("process-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a process-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_process_rules(&table)?;
        config_from_process_rules(&table, &rules)
    })
}

pub fn load_network_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("network-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a network-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_network_rules(&table)?;
        config_from_network_rules(&table, &rules)
    })
}
