use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_source_configs::{config_from_clippy_rules, config_from_unsafe_rules};
use crate::io::{legacy_table_at, read_policy};
use crate::parsers::{is_clippy_exceptions_policy, parse_clippy_rules, parse_unsafe_rules};
use crate::source_context::at_legacy_source;

pub fn load_clippy_exceptions_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy(path)?;
    let result = (|| -> CargoAllowResult<AllowConfig> {
        let table = legacy_table_at(Some(path), &text)?.ok_or_else(|| {
            CargoAllowError::new(format!("{} is not a TOML table", path.display()))
        })?;
        if !is_clippy_exceptions_policy(&table) {
            return Err(CargoAllowError::new(format!(
                "{} is not a clippy-exceptions policy",
                path.display()
            )));
        }
        let rules = parse_clippy_rules(&table)?;
        config_from_clippy_rules(&table, &rules)
    })();
    result.map_err(|err| at_legacy_source(err, path, &text))
}

pub fn load_unsafe_allowlist_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy(path)?;
    let result = (|| -> CargoAllowResult<AllowConfig> {
        let table = legacy_table_at(Some(path), &text)?.ok_or_else(|| {
            CargoAllowError::new(format!("{} is not a TOML table", path.display()))
        })?;
        if table.get("policy").and_then(Value::as_str) != Some("unsafe-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not an unsafe-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_unsafe_rules(&table)?;
        config_from_unsafe_rules(&table, &rules)
    })();
    result.map_err(|err| at_legacy_source(err, path, &text))
}
