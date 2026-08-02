use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_policy_configs::config_from_executable_rules;
use crate::io::legacy_table_at;
use crate::parsers::parse_executable_rules;
use crate::source_context::with_legacy_source;

pub fn load_executable_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("executable-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not an executable-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_executable_rules(&table)?;
        config_from_executable_rules(&table, &rules)
    })
}
