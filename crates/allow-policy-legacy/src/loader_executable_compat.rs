use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_policy_configs::config_from_executable_rules;
use crate::io::{legacy_table_at, read_policy};
use crate::parsers::parse_executable_rules;
use crate::source_context::at_legacy_source;

pub fn load_executable_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy(path)?;
    let result = (|| -> CargoAllowResult<AllowConfig> {
        let table = legacy_table_at(Some(path), &text)?.ok_or_else(|| {
            CargoAllowError::new(format!("{} is not a TOML table", path.display()))
        })?;
        if table.get("policy").and_then(Value::as_str) != Some("executable-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not an executable-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_executable_rules(&table)?;
        config_from_executable_rules(&table, &rules)
    })();
    result.map_err(|err| at_legacy_source(err, path, &text))
}
