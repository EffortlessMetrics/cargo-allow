use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_source_configs::{config_from_clippy_rules, config_from_unsafe_rules};
use crate::io::{legacy_table, read_policy};
pub use crate::loader_file_compat::{load_generated_compat_config, load_non_rust_compat_config};
pub use crate::loader_panic_compat::{
    load_no_panic_allowlist_compat_config, load_no_panic_baseline_compat_config,
};
use crate::parsers::{is_clippy_exceptions_policy, parse_clippy_rules, parse_unsafe_rules};

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
