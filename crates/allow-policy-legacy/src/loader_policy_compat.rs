use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_policy_configs::config_from_executable_rules;
use crate::io::{legacy_table, read_policy};
pub use crate::loader_process_network_compat::{
    load_network_compat_config, load_process_compat_config,
};
pub use crate::loader_workflow_dependency_compat::{
    load_dependency_surface_compat_config, load_workflow_compat_config,
};
use crate::parsers::parse_executable_rules;

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
